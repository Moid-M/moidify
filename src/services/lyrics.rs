//! Lyrics fetching — LRCLIB (word-synced Lyricsfile preferred, then
//! LRC line-sync, then plain) with embedded-tag fallback helpers.
//!
//! LRCLIB rules honored: sequential requests with a 300 ms gap, User-Agent
//! set, and `Retry-After` respected on 429/503.

use std::time::{Duration, Instant};

use lofty::file::TaggedFileExt;
use serde::Deserialize;

static LAST_REQUEST: std::sync::Mutex<Option<Instant>> =
    std::sync::Mutex::new(None);

#[derive(Deserialize, Default)]
struct LyricsFile {
    #[serde(default)]
    lines: Vec<LyricsLine>,
}

#[derive(Deserialize, Default)]
struct LyricsLine {
    #[serde(default)]
    text: String,
    #[serde(default)]
    start_ms: Option<u64>,
    #[serde(default)]
    words: Vec<LyricsWord>,
}

#[derive(Deserialize, Default)]
struct LyricsWord {
    #[serde(default)]
    text: String,
    #[serde(default)]
    start_ms: u64,
}

pub async fn fetch_from_lrclib(
    artist: &str,
    title: &str,
    album: &str,
    duration: Option<f64>,
) -> Option<String> {
    if title.is_empty() && artist.is_empty() {
        return None;
    }

    // Throttle: 300 ms between requests (LRCLIB asks for 200-500 ms).
    let delay = {
        let guard = LAST_REQUEST.lock().unwrap();
        if let Some(prev) = *guard {
            let elapsed = prev.elapsed();
            if elapsed < Duration::from_millis(300) {
                Some(Duration::from_millis(300) - elapsed)
            } else {
                None
            }
        } else {
            None
        }
    };
    if let Some(d) = delay {
        tokio::time::sleep(d).await;
    }
    *LAST_REQUEST.lock().unwrap() = Some(Instant::now());

    let mut url = format!(
        "https://lrclib.net/api/get?artist_name={}&track_name={}&album_name={}",
        urlencode(artist),
        urlencode(title),
        urlencode(album),
    );
    if let Some(d) = duration {
        if d > 0.0 {
            url.push_str(&format!("&duration={}", d.round() as i64));
        }
    }

    let client = reqwest::Client::builder()
        .user_agent("Moidify/1.0 (https://github.com/Moid-M/moidify)")
        .build()
        .ok()?;
    let resp = client
        .get(&url)
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .ok()?;

    if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
        || resp.status() == reqwest::StatusCode::SERVICE_UNAVAILABLE
    {
        if let Some(ra) = resp.headers().get("retry-after") {
            if let Ok(s) = ra.to_str() {
                if let Ok(secs) = s.parse::<u64>() {
                    tokio::time::sleep(Duration::from_secs(secs.min(10))).await;
                }
            }
        }
        return None;
    }
    if !resp.status().is_success() {
        return None;
    }

    let data: serde_json::Value = resp.json().await.ok()?;

    // Prefer word-synced Lyricsfile when available.
    if let Some(lyricsfile) = data.get("lyricsfile").and_then(|v| v.as_str()) {
        if let Some(lrc) = lyricsfile_to_lrc(lyricsfile) {
            return Some(lrc);
        }
    }

    let synced = data.get("syncedLyrics").and_then(|v| v.as_str());
    let plain = data.get("plainLyrics").and_then(|v| v.as_str());
    synced.or(plain).map(|s| s.to_string())
}

/// Convert a LRCLIB Lyricsfile YAML document into enhanced LRC with
/// `<mm:ss.xx>` word tags when word timings are present, otherwise plain
/// LRC line entries.
fn lyricsfile_to_lrc(yaml: &str) -> Option<String> {
    let lf: LyricsFile = serde_yaml::from_str(yaml).ok()?;
    if lf.lines.is_empty() {
        return None;
    }
    let mut out = String::new();
    for line in &lf.lines {
        let start = line.start_ms.unwrap_or(0);
        let (m, s, c) = ms_to_lrc(start);
        if !line.words.is_empty() {
            let mut body = String::new();
            for w in &line.words {
                let (wm, ws, wc) = ms_to_lrc(w.start_ms);
                body.push_str(&format!("<{wm:02}:{ws:02}.{wc:02}>{}", w.text));
            }
            out.push_str(&format!("[{m:02}:{s:02}.{c:02}]{body}\n"));
        } else if !line.text.trim().is_empty() {
            out.push_str(&format!("[{m:02}:{s:02}.{c:02}]{}\n", line.text.trim()));
        }
    }
    if out.trim().is_empty() {
        None
    } else {
        Some(out)
    }
}

fn ms_to_lrc(ms: u64) -> (u64, u64, u64) {
    let m = ms / 60000;
    let s = (ms % 60000) / 1000;
    let c = (ms % 1000) / 10;
    (m, s, c)
}

/// Extract unsynced lyrics from an audio file's tags (USLT/Lyrics).
pub fn extract_embedded_lyrics(path: &str) -> Option<String> {
    let tagged = lofty::read_from_path(path).ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    for key in [lofty::tag::ItemKey::Lyrics, lofty::tag::ItemKey::UnsyncLyrics] {
        if let Some(text) = tag.get_string(key) {
            if !text.trim().is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
