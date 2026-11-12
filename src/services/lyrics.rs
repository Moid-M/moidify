//! LRCLIB lyrics fetch — same endpoint/UA/timeout as Python.

pub async fn fetch_from_lrclib(artist: &str, title: &str, album: &str) -> Option<String> {
    if title.is_empty() && artist.is_empty() {
        return None;
    }
    let url = format!(
        "https://lrclib.net/api/get?artist_name={}&track_name={}&album_name={}",
        urlencode(artist),
        urlencode(title),
        urlencode(album),
    );
    let client = reqwest::Client::builder()
        .user_agent("Moidify/1.0")
        .build()
        .ok()?;
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let data: serde_json::Value = resp.json().await.ok()?;
    let synced = data.get("syncedLyrics").and_then(|v| v.as_str());
    let plain = data.get("plainLyrics").and_then(|v| v.as_str());
    synced.or(plain).map(|s| s.to_string())
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
