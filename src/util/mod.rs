//! Utility functions (normalization, escaping) and security headers.

pub mod security;

/// Unicode NFKD normalization + combining-mark removal (same as Python
/// `_normalize`: NFKD then drop every `unicodedata.category` starting with "M").
pub fn normalize(s: &str) -> String {
    use std::sync::OnceLock;
    static MARKS: OnceLock<regex::Regex> = OnceLock::new();
    let re = MARKS.get_or_init(|| regex::Regex::new(r"\p{M}").expect("mark regex"));
    let nfkd: String = {
        use unicode_normalization::UnicodeNormalization;
        s.nfkd().collect()
    };
    re.replace_all(&nfkd, "").into_owned()
}

/// Escape LIKE wildcards (same as Python `escape_like`).
pub fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_combining_marks() {
        assert_eq!(normalize("café"), "cafe");
        assert_eq!(normalize("naïve"), "naive");
        assert_eq!(normalize("Müller"), "Muller");
        assert_eq!(normalize("plain"), "plain");
    }

    #[test]
    fn normalize_strips_non_latin_marks() {
        // Category-M marks outside the Latin blocks (Python filters all of M*).
        assert_eq!(normalize("Дмитрий\u{486}"), "Дмитрии"); // NFKD: й→и+breve, ҆ dropped
        assert_eq!(normalize("שָׁלוֹם"), "שלום");
        assert_eq!(normalize("Ćirić"), "Ciric");
    }

    #[test]
    fn escape_like_escapes_wildcards() {
        assert_eq!(escape_like("50% off"), "50\\% off");
        assert_eq!(escape_like("a_b"), "a\\_b");
        assert_eq!(escape_like("back\\slash"), "back\\\\slash");
        assert_eq!(escape_like("plain"), "plain");
    }
}
