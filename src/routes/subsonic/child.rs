//! Row → Subsonic child-element mapping — the port of Python's
//! `_track_to_child` / `_album_to_child` / `_artist_to_child` dict builders.
//!
//! Unlike Python (which stringifies nested lists into attribute values — see
//! the module docs in `mod.rs`), children are real nested elements here; the
//! attribute sets, key order, and value formatting stay byte-compatible.

use sqlx::query::Query;
use sqlx::sqlite::{Sqlite, SqliteArguments, SqlitePool, SqliteRow};
use std::path::Path as FsPath;

use super::response::Elem;
use crate::error::{AppError, AppResult};

/// Columns needed to render any track/song child. Handlers `SELECT` this
/// list instead of `*` (Python read whole rows and ignored the rest).
pub const TRACK_COLS: &str = "id, title, artist, album, file_path, year, genre, \
                              duration, track_number, disc_number";

/// Same columns qualified with the `t.` alias for `FROM tracks t JOIN ...`
/// queries — Python used `SELECT t.*`; unqualified names are ambiguous
/// against `play_history.id` / `playlist_tracks.id`.
pub const TRACK_COLS_T: &str = "t.id, t.title, t.artist, t.album, t.file_path, \
                               t.year, t.genre, t.duration, t.track_number, \
                               t.disc_number";

#[derive(Debug, Clone)]
pub struct TrackRow {
    pub id: i64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub file_path: Option<String>,
    pub year: Option<i64>,
    pub genre: Option<String>,
    pub duration: Option<f64>,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
}

impl TrackRow {
    pub fn decode(r: &SqliteRow) -> Self {
        use sqlx::Row;
        Self {
            id: r.get("id"),
            title: r.get("title"),
            artist: r.get("artist"),
            album: r.get("album"),
            file_path: r.get("file_path"),
            year: r.get("year"),
            genre: r.get("genre"),
            duration: r.get("duration"),
            track_number: r.get("track_number"),
            disc_number: r.get("disc_number"),
        }
    }

    /// Python `_track_to_child` — attribute values follow the exact
    /// `x or fallback` truthiness rules (0/None collapse to the fallback).
    pub fn child(&self, tag: &str) -> Elem {
        let size = self.file_size();
        Elem::new(tag)
            .attr("id", self.id)
            .attr("parent", self.id)
            .attr("title", self.title.as_deref().unwrap_or("Unknown"))
            .attr("artist", self.artist.as_deref().unwrap_or("Unknown"))
            .attr("album", self.album.as_deref().unwrap_or("Unknown"))
            .attr("year", nonzero_str(self.year, ""))
            .attr("genre", self.genre.as_deref().unwrap_or(""))
            .attr("size", size)
            .attr("duration", self.duration.unwrap_or(0.0).trunc() as i64)
            .attr("track", nonzero_str(self.track_number, "1"))
            .attr("discNumber", nonzero_str(self.disc_number, "1"))
            .attr("type", "music")
            .attr("suffix", self.suffix())
            .attr("contentType", "audio/mpeg")
            .attr("isDir", "false")
            .attr("path", self.file_path.as_deref().unwrap_or(""))
    }

    /// `os.path.getsize(file_path)` — 0 on any failure or missing path.
    fn file_size(&self) -> u64 {
        let Some(path) = self.file_path.as_deref() else {
            return 0;
        };
        std::fs::metadata(FsPath::new(path))
            .map(|m| m.len())
            .unwrap_or(0)
    }

    /// `Path(file_path).suffix[1:]`, "mp3" when file_path is NULL.
    fn suffix(&self) -> &str {
        match self.file_path.as_deref() {
            Some(p) => FsPath::new(p)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or(""),
            None => "mp3",
        }
    }
}

/// `str(x or fallback)` — Python treats 0 as falsy for ints.
pub fn nonzero_str(v: Option<i64>, fallback: &str) -> String {
    match v {
        Some(n) if n != 0 => n.to_string(),
        _ => fallback.to_string(),
    }
}

/// Grouped album row used by `getAlbum`, `getAlbumList2`, `search3`,
/// `getArtist`, and `getMusicDirectory`. `year`/`genre` only exist in the
/// `getAlbum` query (kept optional for the shared decode).
#[derive(Debug, Clone)]
pub struct AlbumRow {
    pub album: Option<String>,
    pub artist: Option<String>,
    pub track_count: i64,
    pub cover_track_id: Option<i64>,
    pub year: Option<i64>,
    pub genre: Option<String>,
}

impl AlbumRow {
    pub fn decode(r: &SqliteRow) -> Self {
        use sqlx::Row;
        Self {
            album: r.get("album"),
            artist: r.get("artist"),
            track_count: r.get("track_count"),
            cover_track_id: r.get("cover_track_id"),
            year: r.try_get("year").unwrap_or(None),
            genre: r.try_get("genre").unwrap_or(None),
        }
    }

    pub fn album(&self) -> &str {
        // Python `a["album"] or "Unknown"` — empty strings are falsy too.
        match self.album.as_deref() {
            Some(s) if !s.is_empty() => s,
            _ => "Unknown",
        }
    }

    pub fn artist(&self) -> &str {
        match self.artist.as_deref() {
            Some(s) if !s.is_empty() => s,
            _ => "Unknown",
        }
    }

    pub fn id(&self) -> String {
        // Python `md5(f"{album}_{artist}")` — None renders as the literal
        // string "None" in f-strings (getArtist can see NULL albums).
        super::ids::album_id(
            self.album.as_deref().unwrap_or("None"),
            self.artist.as_deref().unwrap_or("None"),
        )
    }

    /// Python `_album_to_child` (used by `getAlbumList2` / `search3`).
    pub fn child(&self) -> Elem {
        let id = self.id();
        Elem::new("album")
            .attr("id", &id)
            .attr("parent", &id)
            .attr("title", self.album())
            .attr("artist", self.artist())
            .attr("year", "")
            .attr("genre", "")
            .attr("duration", "0")
            .attr("track", "1")
            .attr("discNumber", "1")
            .attr("type", "album")
            .attr("suffix", "")
            .attr("contentType", "")
            .attr("isDir", "true")
            // Python `f"{album['album']}"` — NULL renders as the literal "None".
            .attr("path", self.album.as_deref().unwrap_or("None"))
            .attr(
                "coverArt",
                self.cover_track_id
                    .map(|c| c.to_string())
                    .unwrap_or_default(),
            )
            .attr("songCount", self.track_count)
            .attr("album", self.album.as_deref().unwrap_or(""))
    }

    /// Album child variant used under `<artist>` in `getArtist` (has
    /// `created`, no track/discNumber attributes). `parent` is the requested
    /// artist id.
    pub fn browse_child(&self, parent: &str) -> Elem {
        Elem::new("album")
            .attr("id", self.id())
            .attr("parent", parent)
            .attr("title", self.album())
            .attr("artist", self.artist())
            .attr("isDir", "true")
            .attr(
                "coverArt",
                self.cover_track_id
                    .map(|c| c.to_string())
                    .unwrap_or_default(),
            )
            .attr("songCount", self.track_count)
            .attr("duration", "0")
            .attr("created", "")
            .attr("year", "")
            .attr("genre", "")
            .attr("type", "album")
            .attr("suffix", "")
            .attr("contentType", "")
            .attr("path", self.album.as_deref().unwrap_or(""))
            .attr("album", self.album.as_deref().unwrap_or(""))
    }

    /// Minimal album child variant used under `<directory>` for an artist in
    /// `getMusicDirectory` — Python's dict key there is "child" (and the
    /// Subsonic spec renders directory children as `<child .../>`).
    pub fn directory_child(&self, parent: &str) -> Elem {
        Elem::new("child")
            .attr("id", self.id())
            .attr("parent", parent)
            .attr("title", self.album())
            .attr("artist", self.artist())
            .attr("isDir", "true")
            .attr(
                "coverArt",
                self.cover_track_id
                    .map(|c| c.to_string())
                    .unwrap_or_default(),
            )
            .attr("songCount", self.track_count)
            .attr("type", "album")
            .attr("path", self.album.as_deref().unwrap_or(""))
            .attr("album", self.album.as_deref().unwrap_or(""))
    }
}

/// Grouped artist row (`getArtists` / `search3`).
#[derive(Debug, Clone)]
pub struct ArtistRow {
    pub artist: Option<String>,
    pub track_count: i64,
    pub album_count: i64,
}

impl ArtistRow {
    pub fn decode(r: &SqliteRow) -> Self {
        use sqlx::Row;
        Self {
            artist: r.get("artist"),
            track_count: r.get("track_count"),
            album_count: r.get("album_count"),
        }
    }

    /// Python `_artist_to_child`.
    pub fn child(&self) -> Elem {
        let name = self.artist.as_deref().unwrap_or("");
        Elem::new("artist")
            .attr("id", super::ids::artist_id(name))
            .attr("name", name)
            .attr("albumCount", self.album_count)
            .attr("coverArt", "")
    }
}

/// Playlist row for `getPlaylists` (`track_count` from the LEFT JOIN).
#[derive(Debug, Clone)]
pub struct PlaylistRow {
    pub id: i64,
    pub name: Option<String>,
    pub track_count: Option<i64>,
}

impl PlaylistRow {
    pub fn decode(r: &SqliteRow) -> Self {
        use sqlx::Row;
        Self {
            id: r.get("id"),
            name: r.get("name"),
            track_count: r.try_get("track_count").unwrap_or(None),
        }
    }

    /// Python playlist child (owner injected by the caller — it comes from
    /// the authenticated user, not the row).
    pub fn child(&self, owner: &str) -> Elem {
        let name = match self.name.as_deref() {
            Some(s) if !s.is_empty() => s,
            _ => "Unnamed",
        };
        Elem::new("playlist")
            .attr("id", self.id)
            .attr("name", name)
            .attr("owner", owner)
            .attr("public", "false")
            .attr("songCount", self.track_count.unwrap_or(0))
            .attr("duration", "0")
            .attr("created", "")
    }
}

/// Fetch all rows of a prepared query and decode them as track rows.
pub async fn fetch_tracks<'q>(
    pool: &SqlitePool,
    q: Query<'q, Sqlite, SqliteArguments<'q>>,
) -> AppResult<Vec<TrackRow>> {
    let rows = q
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(rows.iter().map(TrackRow::decode).collect())
}

/// Fetch at most one track row.
pub async fn fetch_track<'q>(
    pool: &SqlitePool,
    q: Query<'q, Sqlite, SqliteArguments<'q>>,
) -> AppResult<Option<TrackRow>> {
    let row = q
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(row.as_ref().map(TrackRow::decode))
}

/// Fetch grouped album rows (`getAlbum` / `getArtist` / `search3` / …).
pub async fn fetch_albums<'q>(
    pool: &SqlitePool,
    q: Query<'q, Sqlite, SqliteArguments<'q>>,
) -> AppResult<Vec<AlbumRow>> {
    let rows = q
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(rows.iter().map(AlbumRow::decode).collect())
}

/// Fetch grouped artist rows (`getArtists` / `search3`).
pub async fn fetch_artists<'q>(
    pool: &SqlitePool,
    q: Query<'q, Sqlite, SqliteArguments<'q>>,
) -> AppResult<Vec<ArtistRow>> {
    let rows = q
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(rows.iter().map(ArtistRow::decode).collect())
}

/// Fetch playlist rows (`getPlaylists`).
pub async fn fetch_playlists<'q>(
    pool: &SqlitePool,
    q: Query<'q, Sqlite, SqliteArguments<'q>>,
) -> AppResult<Vec<PlaylistRow>> {
    let rows = q
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(rows.iter().map(PlaylistRow::decode).collect())
}

/// Fetch a single playlist row (`getPlaylist`).
pub async fn fetch_playlist<'q>(
    pool: &SqlitePool,
    q: Query<'q, Sqlite, SqliteArguments<'q>>,
) -> AppResult<Option<(i64, String)>> {
    let row = q
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(row.map(|r| {
        use sqlx::Row;
        (r.get("id"), r.get("name"))
    }))
}

/// Python `str(sum(d or 0 ...))` over album/playlist durations — renders
/// `0` when every term collapsed to the falsy fallback, otherwise the
/// shortest float repr with `str()` semantics (e.g. `120.0`, `120.467`).
pub fn duration_sum_str(durations: &[Option<f64>]) -> String {
    let has_value = durations.iter().any(|d| matches!(d, Some(v) if *v != 0.0));
    if !has_value {
        return "0".to_string();
    }
    let sum: f64 = durations.iter().map(|d| d.unwrap_or(0.0)).sum();
    py_f64_str(sum)
}

/// Python `str(float)` for typical magnitudes: always keeps one decimal
/// place for integral values (`241.0`), otherwise shortest round-trip.
pub fn py_f64_str(v: f64) -> String {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 1e16 {
        format!("{v:.1}")
    } else {
        format!("{v}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_sum_parity() {
        assert_eq!(duration_sum_str(&[]), "0");
        assert_eq!(duration_sum_str(&[None, None]), "0");
        assert_eq!(duration_sum_str(&[Some(0.0), None]), "0");
        assert_eq!(duration_sum_str(&[Some(120.467)]), "120.467");
        assert_eq!(duration_sum_str(&[Some(120.0), None]), "120.0");
        assert_eq!(duration_sum_str(&[Some(0.0), Some(121.5)]), "121.5");
    }

    #[test]
    fn py_f64_str_parity() {
        assert_eq!(py_f64_str(241.0), "241.0");
        assert_eq!(py_f64_str(0.0), "0.0");
        assert_eq!(py_f64_str(120.467), "120.467");
        assert_eq!(py_f64_str(0.1 + 0.2), "0.30000000000000004");
    }

    #[test]
    fn nonzero_str_parity() {
        assert_eq!(nonzero_str(None, ""), "");
        assert_eq!(nonzero_str(Some(0), ""), "");
        assert_eq!(nonzero_str(Some(2020), ""), "2020");
        assert_eq!(nonzero_str(None, "1"), "1");
        assert_eq!(nonzero_str(Some(0), "1"), "1");
        assert_eq!(nonzero_str(Some(3), "1"), "3");
    }
}
