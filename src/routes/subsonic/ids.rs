//! Subsonic id scheme — same as Python `subsonic.py`:
//! songs use the plain database id, while artist/album directories use a
//! truncated MD5 so names with quotes/slashes survive round-trips.

/// Raw lowercase MD5 hex digest of `s`.
pub fn md5_hex(s: &str) -> String {
    use md5::Digest;
    format!("{:x}", md5::Md5::digest(s.as_bytes()))
}

/// `ar_` + first 12 hex chars of `md5(artist_name)`.
pub fn artist_id(name: &str) -> String {
    format!("ar_{}", &md5_hex(name)[..12])
}

/// `al_` + first 12 hex chars of `md5("{album}_{artist}")` where `artist` is
/// the same COALESCE(NULLIF(album_artist,''), artist) value used in queries.
pub fn album_id(album: &str, artist: &str) -> String {
    format!("al_{}", &md5_hex(&format!("{album}_{artist}"))[..12])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artist_id_scheme() {
        assert_eq!(artist_id("Alpha Band"), "ar_899894a0846e");
        assert_eq!(artist_id("Alpha Band").len(), 3 + 12);
    }

    #[test]
    fn album_id_scheme() {
        // Matches Python: md5(f"{album}_{artist}").hexdigest()[:12].
        assert_eq!(album_id("First Album", "Alpha Band"), "al_7f97e0a6bf2d");
    }

    #[test]
    fn md5_hex_matches_python_hashlib() {
        assert_eq!(md5_hex(""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex("abc"), "900150983cd24fb0d6963f7d28e17f72");
    }
}
