//! Deezer artist image fetch + disk cache — same as Python `get_artist_image`.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::path::Path;

/// Try Deezer search → download → cache to `covers/artist_<key><ext>`.
/// Returns `None` on any failure (caller falls back to gradient SVG).
pub async fn fetch_deezer(
    artist_name: &str,
    cache_key: &str,
    covers_dir: &Path,
) -> Option<Response> {
    let client = reqwest::Client::builder()
        .user_agent("Moidify/1.0")
        .build()
        .ok()?;

    let search_url = format!(
        "https://api.deezer.com/search/artist?q={}&limit=1",
        urlencode(artist_name)
    );
    let resp = client
        .get(&search_url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let data: serde_json::Value = resp.json().await.ok()?;
    let first = data.get("data")?.as_array()?.first()?;
    let picture_url = first
        .get("picture_medium")
        .and_then(|v| v.as_str())
        .or_else(|| first.get("picture").and_then(|v| v.as_str()))?;

    let img = client
        .get(picture_url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .ok()?
        .bytes()
        .await
        .ok()?;

    let ext = if img.starts_with(b"\x89PNG") {
        "png"
    } else if img.starts_with(b"RIFF") && img.len() > 12 && &img[8..12] == b"WEBP" {
        "webp"
    } else {
        "jpg"
    };
    let mime = match ext {
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "image/jpeg",
    };

    let path = covers_dir.join(format!("artist_{cache_key}.{ext}"));
    tokio::fs::write(&path, &img).await.ok()?;

    Some(
        (
            StatusCode::OK,
            [
                ("content-type", mime.to_string()),
                ("cache-control", "public, max-age=86400".to_string()),
            ],
            img.to_vec(),
        )
            .into_response(),
    )
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
