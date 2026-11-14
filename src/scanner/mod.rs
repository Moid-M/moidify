//! Music library scanner — port of Python `scanner.py`.
//!
//! * metadata via **lofty** (Python: mutagen)
//! * filesystem watcher via **notify** (Python: watchdog)
//! * `file_hash`, cover dedup, `.lrc` sidecars, full/clean scan, rescan
//!   scheduler — byte-compatible with the Python behaviour, including its
//!   quirks (e.g. a pre-existing track skips re-extraction unless `force`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::Local;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::{Accessor, ItemKey, Tag};
use notify::event::{ModifyKind, RenameMode};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::state::AppState;

pub const AUDIO_EXTENSIONS: [&str; 8] = ["mp3", "flac", "ogg", "m4a", "wav", "wma", "aac", "opus"];
const MAX_ERRORS: usize = 100;

// ── Status ────────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct ScanStatus {
    pub last_scan: Option<String>,
    pub files_found: i64,
    pub files_imported: i64,
    pub errors: Vec<String>,
}

/// Shared scan state (Python: `SCAN_STATUS` + `_SCAN_RUNNING` globals).
pub struct Scanner {
    pub status: tokio::sync::Mutex<ScanStatus>,
    pub running: AtomicBool,
    /// Keeps the notify watcher alive for the lifetime of this state.
    watcher: std::sync::Mutex<Option<RecommendedWatcher>>,
}

impl Scanner {
    pub fn new() -> Self {
        Self {
            status: tokio::sync::Mutex::new(ScanStatus::default()),
            running: AtomicBool::new(false),
            watcher: std::sync::Mutex::new(None),
        }
    }

    /// Python `get_scan_status()`.
    pub async fn status_json(&self) -> Value {
        let st = self.status.lock().await;
        json!({
            "running": self.running.load(Ordering::SeqCst),
            "last_scan": st.last_scan,
            "files_found": st.files_found,
            "files_imported": st.files_imported,
            "errors": st.errors,
        })
    }
}

impl Default for Scanner {
    fn default() -> Self {
        Self::new()
    }
}

fn push_error(st: &mut ScanStatus, msg: String) {
    st.errors.push(msg);
    if st.errors.len() > MAX_ERRORS {
        let len = st.errors.len();
        st.errors.drain(..len - MAX_ERRORS);
    }
}

fn now_str() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Reset `running` even on early return / panic (Python `finally`).
struct RunningGuard<'a>(&'a AtomicBool);

impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

// ── Metadata extraction ──────────────────────────────────────────────────────

pub struct Metadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
    pub genre: Option<String>,
    pub year: Option<i64>,
    pub duration: Option<i64>,
    pub cover_data: Option<Vec<u8>>,
    pub lyrics: Option<String>,
}

fn clean(v: Option<impl AsRef<str>>) -> Option<String> {
    v.map(|s| s.as_ref().trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Python: `int(tn.split('/')[0])` for TRCK/TPOS-style values.
fn parse_int_frac(v: Option<&str>) -> Option<i64> {
    let v = v?;
    let first = v.split('/').next().unwrap_or("");
    first.trim().parse::<i64>().ok()
}

/// Python: `re.search(r'\b(\d{4})\b', str(year))`.
fn parse_year(v: &str) -> Option<i64> {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\b(\d{4})\b").expect("valid regex"));
    re.captures(v)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<i64>().ok())
}

/// Lyrics: Python reads USLT (MP3), `lyrics`/`unsyncedlyrics` (FLAC/OGG),
/// `©lyr` (MP4). lofty maps all of these to `Lyrics`/`UnsyncLyrics`.
fn extract_lyrics(tag: &Tag) -> Option<String> {
    tag.get_string(ItemKey::Lyrics)
        .or_else(|| tag.get_string(ItemKey::UnsyncLyrics))
        .map(str::to_string)
}

/// Python `extract_metadata()` — `None` means the file couldn't be parsed at
/// all (mutagen `File()` returned None / raised), which triggers the caller's
/// "Unknown" fallback dict.
pub fn extract_metadata(path: &Path) -> Option<Metadata> {
    let tagged = lofty::read_from_path(path).ok()?;
    let duration = tagged.properties().duration().as_secs() as i64;

    let mut info = Metadata {
        title: None,
        artist: None,
        album_artist: None,
        album: None,
        track_number: None,
        disc_number: None,
        genre: None,
        year: None,
        duration: Some(duration),
        cover_data: None,
        lyrics: None,
    };

    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    if let Some(tag) = tag {
        info.title = clean(tag.title());
        info.artist = clean(tag.artist());
        info.album = clean(tag.album());
        info.album_artist = clean(tag.get_string(ItemKey::AlbumArtist));
        info.track_number = parse_int_frac(tag.get_string(ItemKey::TrackNumber));
        info.disc_number = parse_int_frac(tag.get_string(ItemKey::DiscNumber));
        info.genre = clean(tag.genre());
        // Python reads TDRC (MP3) / date (FLAC/OGG) / ©day (MP4).
        info.year = tag
            .get_string(ItemKey::RecordingDate)
            .or_else(|| tag.get_string(ItemKey::Year))
            .or_else(|| tag.get_string(ItemKey::ReleaseDate))
            .or_else(|| tag.get_string(ItemKey::OriginalReleaseDate))
            .and_then(parse_year);
        info.lyrics = extract_lyrics(tag).and_then(|l| clean(Some(&l)));
        info.cover_data = tag.pictures().first().map(|p| p.data().to_vec());
    }

    // Python: title falls back to the file stem when empty.
    if info.title.is_none() {
        info.title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string);
    }

    // Companion .lrc sidecar (only reached when the file parsed — Python
    // reads it before returning, but not on the parse-failure path).
    if info.lyrics.is_none() {
        let lrc = path.with_extension("lrc");
        if lrc.is_file() {
            // Python: read_text(encoding="utf-8", errors="replace")
            info.lyrics = std::fs::read(&lrc)
                .ok()
                .map(|b| String::from_utf8_lossy(&b).into_owned());
        }
    }

    Some(info)
}

/// Fallback when `extract_metadata` returns None (Python's inline dict).
fn fallback_metadata(path: &Path) -> Metadata {
    Metadata {
        title: path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string),
        artist: Some("Unknown".to_string()),
        album_artist: None,
        album: Some("Unknown".to_string()),
        track_number: None,
        disc_number: None,
        genre: None,
        year: None,
        duration: Some(0),
        cover_data: None,
        lyrics: None,
    }
}

// ── File hash / covers ───────────────────────────────────────────────────────

/// Python `get_file_hash()`: SHA-256 over (first 64 KiB + last 64 KiB for
/// files > 128 KiB, otherwise the whole file) followed by the decimal size.
pub fn get_file_hash(path: &Path) -> std::io::Result<String> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = std::fs::File::open(path)?;
    let size = file.metadata()?.len();
    let mut hasher = Sha256::new();
    if size > 131_072 {
        let mut buf = [0u8; 65_536];
        file.read_exact(&mut buf)?;
        hasher.update(buf);
        file.seek(SeekFrom::End(-65_536))?;
        file.read_exact(&mut buf)?;
        hasher.update(buf);
    } else {
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        hasher.update(&data);
    }
    hasher.update(size.to_string().as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

/// Python `_save_cover()` — sniff the container, dedupe by content hash.
fn save_cover(covers_dir: &Path, data: &[u8]) -> std::io::Result<String> {
    let ext = if data.starts_with(b"\x89PNG") {
        ".png"
    } else if data.starts_with(b"\xff\xd8") {
        ".jpg"
    } else if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        ".webp"
    } else {
        ".jpg"
    };
    let full = hex::encode(Sha256::digest(data));
    let cover_hash = &full[..16];
    let cover_path = covers_dir.join(format!("{cover_hash}{ext}"));
    if !cover_path.exists() {
        std::fs::write(&cover_path, data)?;
    }
    Ok(cover_hash.to_string())
}

// ── process_file ─────────────────────────────────────────────────────────────

pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Python `process_file()` — `Err(msg)` means the message must be appended to
/// `SCAN_STATUS["errors"]` (Python appends inside the function).
pub async fn process_file(app: &AppState, path: &Path, force: bool) -> Result<(), String> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    if !is_audio(path) {
        return Ok(());
    }
    let path_str = path.to_string_lossy().to_string();

    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM tracks WHERE file_path = ?")
        .bind(&path_str)
        .fetch_optional(&app.pool)
        .await
        .map_err(|e| format!("Error processing: {name}: {e}"))?;
    if existing.is_some() && !force {
        return Ok(());
    }

    let metadata = extract_metadata(path).unwrap_or_else(|| fallback_metadata(path));

    let file_hash = get_file_hash(path).map_err(|_| format!("Failed to hash: {name}"))?;

    let cover_hash = match &metadata.cover_data {
        Some(data) => Some(
            save_cover(&app.config.covers_dir, data)
                .map_err(|e| format!("Error processing: {name}: {e}"))?,
        ),
        None => None,
    };
    let has_cover = metadata.cover_data.is_some() as i64;

    let result = if existing.is_some() && force {
        sqlx::query(
            r#"UPDATE tracks SET
               file_hash=?, title=?, artist=?, album_artist=?, album=?, track_number=?,
               disc_number=?, genre=?, year=?, duration=?, has_cover=?, lyrics=?, cover_hash=?
               WHERE file_path=?"#,
        )
        .bind(&file_hash)
        .bind(&metadata.title)
        .bind(&metadata.artist)
        .bind(&metadata.album_artist)
        .bind(&metadata.album)
        .bind(metadata.track_number)
        .bind(metadata.disc_number)
        .bind(&metadata.genre)
        .bind(metadata.year)
        .bind(metadata.duration)
        .bind(has_cover)
        .bind(&metadata.lyrics)
        .bind(&cover_hash)
        .bind(&path_str)
        .execute(&app.pool)
        .await
    } else {
        sqlx::query(
            r#"INSERT OR IGNORE INTO tracks
               (file_path, file_hash, title, artist, album_artist, album, track_number,
                disc_number, genre, year, duration, has_cover, lyrics, cover_hash)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&path_str)
        .bind(&file_hash)
        .bind(&metadata.title)
        .bind(&metadata.artist)
        .bind(&metadata.album_artist)
        .bind(&metadata.album)
        .bind(metadata.track_number)
        .bind(metadata.disc_number)
        .bind(&metadata.genre)
        .bind(metadata.year)
        .bind(metadata.duration)
        .bind(has_cover)
        .bind(&metadata.lyrics)
        .bind(&cover_hash)
        .execute(&app.pool)
        .await
    };

    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Error processing: {name}: {e}")),
    }
}

// ── scan_existing ────────────────────────────────────────────────────────────

fn collect_audio_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_audio_files(&path, out)?;
        } else if is_audio(&path) {
            out.push(path);
        }
    }
    Ok(())
}

/// Python `scan_existing(clean=False)` — synchronous scan, guarded so a
/// second call while running is a no-op.
pub async fn scan_existing(app: &AppState, clean: bool) {
    if app.scanner.running.swap(true, Ordering::SeqCst) {
        return;
    }
    let _guard = RunningGuard(&app.scanner.running);

    if !app.config.music_dir.exists() {
        let mut st = app.scanner.status.lock().await;
        st.last_scan = Some(now_str());
        push_error(&mut st, "Music directory does not exist".to_string());
        return;
    }

    {
        let mut st = app.scanner.status.lock().await;
        st.errors.clear();
        st.files_found = 0;
        st.files_imported = 0;
    }

    if clean {
        for stmt in [
            "DELETE FROM playlist_tracks",
            "DELETE FROM favorites",
            "DELETE FROM tracks",
        ] {
            let _ = sqlx::query(stmt).execute(&app.pool).await;
        }
        if let Ok(rd) = std::fs::read_dir(&app.config.covers_dir) {
            for entry in rd.flatten() {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    let mut files = Vec::new();
    if let Err(e) = collect_audio_files(&app.config.music_dir, &mut files) {
        let mut st = app.scanner.status.lock().await;
        push_error(&mut st, format!("Scan error: {e}"));
    }

    for file in files {
        {
            let mut st = app.scanner.status.lock().await;
            st.files_found += 1;
        }
        let was = {
            let st = app.scanner.status.lock().await;
            st.errors.len()
        };
        if let Err(msg) = process_file(app, &file, clean).await {
            let mut st = app.scanner.status.lock().await;
            push_error(&mut st, msg);
        }
        let mut st = app.scanner.status.lock().await;
        if st.errors.len() == was {
            st.files_imported += 1;
        }
        // Python commits every 50 files; sqlx autocommits per statement.
    }

    let mut st = app.scanner.status.lock().await;
    st.last_scan = Some(now_str());
}

// ── Rescan schedule ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Schedule {
    /// Python `ScheduleBody.interval_hours: float`.
    #[serde(default)]
    pub interval_hours: f64,
    #[serde(default)]
    pub last_scan: Option<String>,
}

/// Python `SCHEDULE_FILE = BASE_DIR / "data" / "rescan_schedule.json"` —
/// kept next to the DB (default layout is identical: `data/`).
pub fn schedule_path(app: &AppState) -> PathBuf {
    app.config
        .db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("rescan_schedule.json")
}

pub fn load_schedule(path: &Path) -> Schedule {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_schedule(path: &Path, schedule: &Schedule) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string(schedule).expect("serializable"))
}

/// Python `start_rescan_scheduler()` — one loop per process; re-reads the
/// schedule file every cycle so endpoint edits take effect.
pub fn spawn_rescan_scheduler(app: AppState) {
    tokio::spawn(async move {
        let path = schedule_path(&app);
        loop {
            let schedule = load_schedule(&path);
            if schedule.interval_hours > 0.0 {
                scan_existing(&app, false).await;
                let mut updated = schedule;
                updated.last_scan = Some(now_str());
                let _ = save_schedule(&path, &updated);
                tokio::time::sleep(Duration::from_secs(
                    (updated.interval_hours * 3600.0) as u64,
                ))
                .await;
            } else {
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        }
    });
}

// ── Watcher (Python: watchdog Observer) ──────────────────────────────────────

/// Find the audio sibling of a `.lrc` file (Python `with_suffix` chain).
fn lrc_audio_sibling(lrc: &Path) -> Option<PathBuf> {
    let base = lrc.with_extension("");
    for ext in AUDIO_EXTENSIONS {
        let candidate = base.with_extension(ext);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

async fn on_create(app: &AppState, path: PathBuf) {
    if path.is_dir() {
        return;
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();

    if ext == "lrc" {
        // Python: sleep 1s, then process the sibling (no counters).
        if let Some(audio) = lrc_audio_sibling(&path) {
            let app = app.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let _ = process_file(&app, &audio, false).await;
            });
        }
        return;
    }
    if !is_audio(&path) {
        return;
    }

    // Python sleeps 2s to let the writer finish before hashing.
    let app = app.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        {
            let mut st = app.scanner.status.lock().await;
            st.files_found += 1;
        }
        let was = {
            let st = app.scanner.status.lock().await;
            st.errors.len()
        };
        if let Err(msg) = process_file(&app, &path, false).await {
            let mut st = app.scanner.status.lock().await;
            push_error(&mut st, msg);
        }
        let mut st = app.scanner.status.lock().await;
        if st.errors.len() == was {
            st.files_imported += 1;
        }
        st.last_scan = Some(now_str());
    });
}

async fn on_remove(app: &AppState, path: &Path) {
    if !is_audio(path) {
        return;
    }
    let path_str = path.to_string_lossy().to_string();
    let _ = sqlx::query("DELETE FROM tracks WHERE file_path = ?")
        .bind(path_str)
        .execute(&app.pool)
        .await;
}

/// Python `MusicFileHandler` — ignore events while a full scan is running.
async fn handle_event(app: &AppState, event: Event) {
    if app.scanner.running.load(Ordering::SeqCst) {
        return;
    }
    match event.kind {
        EventKind::Create(_) => {
            for path in event.paths {
                on_create(app, path).await;
            }
        }
        EventKind::Remove(_) => {
            for path in event.paths {
                on_remove(app, &path).await;
            }
        }
        EventKind::Modify(ModifyKind::Name(mode)) => {
            let paths = event.paths;
            let (src, dst) = match mode {
                RenameMode::From => (paths.first().cloned(), None),
                RenameMode::To => (None, paths.first().cloned()),
                _ => (paths.first().cloned(), paths.get(1).cloned()),
            };
            if let Some(src) = src {
                // Renamed away: drop the old row (or re-process a moved .lrc).
                if src
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("lrc"))
                    == Some(true)
                {
                    if let Some(audio) = lrc_audio_sibling(&src) {
                        let _ = process_file(app, &audio, false).await;
                    }
                } else {
                    on_remove(app, &src).await;
                }
            }
            if let Some(dst) = dst {
                on_create(app, dst).await;
            }
        }
        _ => {}
    }
}

/// Python `start_watcher()` — recursive watch on the music dir.
pub fn spawn_watcher(app: &AppState) -> notify::Result<()> {
    let runtime = tokio::runtime::Handle::current();
    let watched = app.clone();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        let Ok(event) = res else { return };
        let app = watched.clone();
        let runtime = runtime.clone();
        runtime.spawn(async move {
            handle_event(&app, event).await;
        });
    })?;

    if app.config.music_dir.exists() {
        watcher.watch(&app.config.music_dir, RecursiveMode::Recursive)?;
    } else {
        tracing::warn!(
            path = %app.config.music_dir.display(),
            "music directory missing — file watcher not started"
        );
        return Ok(());
    }

    *app.scanner.watcher.lock().expect("watcher lock") = Some(watcher);
    Ok(())
}

// ── Startup (Python: server.py lifespan) ─────────────────────────────────────

/// Initial background scan + watcher + rescan scheduler.
pub fn spawn_startup(app: AppState) {
    let for_scan = app.clone();
    tokio::spawn(async move {
        scan_existing(&for_scan, false).await;
    });
    if let Err(e) = spawn_watcher(&app) {
        tracing::warn!(error = %e, "failed to start file watcher");
    }
    spawn_rescan_scheduler(app);
}

// ── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_int_frac_variants() {
        assert_eq!(parse_int_frac(Some("3/12")), Some(3));
        assert_eq!(parse_int_frac(Some("3")), Some(3));
        assert_eq!(parse_int_frac(Some(" 7 ")), Some(7));
        assert_eq!(parse_int_frac(Some("abc")), None);
        assert_eq!(parse_int_frac(Some("3.0")), None);
        assert_eq!(parse_int_frac(Some("")), None);
        assert_eq!(parse_int_frac(None), None);
    }

    #[test]
    fn parse_year_variants() {
        assert_eq!(parse_year("2019"), Some(2019));
        assert_eq!(parse_year("2019-05-01"), Some(2019));
        assert_eq!(parse_year("May 12, 2005"), Some(2005));
        assert_eq!(parse_year("no year"), None);
        // Python \b(\d{4})\b does not match inside a longer digit run.
        assert_eq!(parse_year("12019"), None);
    }

    #[test]
    fn is_audio_extensions() {
        assert!(is_audio(Path::new("/x/song.MP3")));
        assert!(is_audio(Path::new("/x/song.flac")));
        assert!(!is_audio(Path::new("/x/cover.jpg")));
        assert!(!is_audio(Path::new("/x/notes.txt")));
    }

    #[test]
    fn push_error_caps_at_100() {
        let mut st = ScanStatus::default();
        for i in 0..150 {
            push_error(&mut st, format!("err {i}"));
        }
        assert_eq!(st.errors.len(), MAX_ERRORS);
        assert_eq!(st.errors.first().unwrap(), "err 50");
        assert_eq!(st.errors.last().unwrap(), "err 149");
    }

    #[test]
    fn schedule_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rescan_schedule.json");
        // Missing file → Python default.
        let s = load_schedule(&path);
        assert_eq!(s.interval_hours, 0.0);
        assert!(s.last_scan.is_none());

        let s = Schedule {
            interval_hours: 8.0,
            last_scan: Some("2026-01-01 00:00:00".into()),
        };
        save_schedule(&path, &s).unwrap();
        let back = load_schedule(&path);
        assert_eq!(back.interval_hours, 8.0);
        assert_eq!(back.last_scan.as_deref(), Some("2026-01-01 00:00:00"));

        // Corrupt file → default (Python swallows JSON errors).
        std::fs::write(&path, "{not json").unwrap();
        let s = load_schedule(&path);
        assert_eq!(s.interval_hours, 0.0);
    }
}
