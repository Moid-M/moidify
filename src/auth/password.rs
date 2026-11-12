//! PBKDF2-SHA256 password hashing — byte-compatible with the Python server
//! (`hashlib.pbkdf2_hmac("sha256", pw, salt, 600_000).hex()`).

use sha2::Sha256;

pub const PBKDF2_ITERATIONS: u32 = 600_000;

/// Generate a random salt (32 hex chars, same as `secrets.token_hex(16)`).
pub fn generate_salt() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Derive the hex PBKDF2 hash for `password` under `salt`.
pub fn hash_password(password: &str, salt: &str) -> String {
    let mut out = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(
        password.as_bytes(),
        salt.as_bytes(),
        PBKDF2_ITERATIONS,
        &mut out,
    );
    hex::encode(out)
}

/// Async wrapper — PBKDF2-600k is CPU-heavy and would block tokio workers.
pub async fn hash_password_async(password: String, salt: String) -> String {
    tokio::task::spawn_blocking(move || hash_password(&password, &salt))
        .await
        .expect("pbkdf2 task")
}

/// Async verify — see [`hash_password_async`].
pub async fn verify_password_async(password: String, salt: String, expected_hash: String) -> bool {
    tokio::task::spawn_blocking(move || verify_password(&password, &salt, &expected_hash))
        .await
        .expect("pbkdf2 task")
}

/// Create a new (hash, salt) pair.
pub fn new_password(password: &str) -> (String, String) {
    let salt = generate_salt();
    let hash = hash_password(password, &salt);
    (hash, salt)
}

/// Async variant of [`new_password`].
pub async fn new_password_async(password: String) -> (String, String) {
    let salt = generate_salt();
    let hash = hash_password_async(password, salt.clone()).await;
    (hash, salt)
}

/// Timing-safe password verification (mirrors `_verify_password`).
pub fn verify_password(password: &str, salt: &str, expected_hash: &str) -> bool {
    if salt.is_empty() || expected_hash.is_empty() {
        return false;
    }
    let candidate = hash_password(password, salt);
    // hmac::Mac-based compare for constant time; hex strings are equal length.
    let a = candidate.as_bytes();
    let b = expected_hash.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Password policy — same rules and messages as Python `_validate_password`.
pub fn validate_password(pwd: &str) -> Result<(), String> {
    if pwd.chars().count() < 8 {
        return Err("Password must be at least 8 characters".into());
    }
    if pwd.chars().count() > 128 {
        return Err("Password must be at most 128 characters".into());
    }
    if !pwd.chars().any(|c| c.is_ascii_lowercase()) {
        return Err("Password must contain a lowercase letter".into());
    }
    if !pwd.chars().any(|c| c.is_ascii_uppercase()) {
        return Err("Password must contain an uppercase letter".into());
    }
    if !pwd.chars().any(|c| c.is_ascii_digit()) {
        return Err("Password must contain a digit".into());
    }
    const SPECIALS: &[char] = &[
        '!', '@', '#', '$', '%', '^', '&', '*', '(', ')', ',', '.', '?', '"', ':', '{', '}', '|',
        '<', '>', '_', '-',
    ];
    if !pwd.chars().any(|c| SPECIALS.contains(&c)) {
        return Err("Password must contain a special character".into());
    }
    Ok(())
}

/// Email format check (same regex intent as Python).
pub fn validate_email(email: &str) -> Result<(), String> {
    let e = email.trim();
    if e.is_empty() {
        return Ok(());
    }
    // ^[^@\s]+@[^@\s]+\.[^@\s]+$
    let bytes = e.as_bytes();
    let mut seen_at = false;
    let mut seen_dot_after_at = false;
    let mut local = 0usize;
    let mut domain = 0usize;
    let mut after_dot = 0usize;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'@' if !seen_at => {
                if local == 0 {
                    return Err("Invalid email format".into());
                }
                seen_at = true;
            }
            b'.' if seen_at => {
                if domain == 0 {
                    return Err("Invalid email format".into());
                }
                seen_dot_after_at = true;
                after_dot = 0;
            }
            b if b.is_ascii_whitespace() => return Err("Invalid email format".into()),
            _ => {
                if !seen_at {
                    local += 1;
                } else {
                    domain += 1;
                    if seen_dot_after_at {
                        after_dot += 1;
                    }
                }
            }
        }
        let _ = i;
    }
    if !seen_at || !seen_dot_after_at || after_dot == 0 || domain == 0 {
        return Err("Invalid email format".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_verify_roundtrip() {
        let (h, s) = new_password("TestPass123!");
        assert!(verify_password("TestPass123!", &s, &h));
        assert!(!verify_password("wrong", &s, &h));
    }

    #[test]
    fn hash_is_not_plaintext() {
        let (h, s) = new_password("TestPass123!");
        assert!(!h.contains("TestPass123!"));
        assert!(!s.contains("TestPass123!"));
        assert_eq!(h.len(), 64); // sha256 hex
        assert_eq!(s.len(), 32); // token_hex(16)
    }

    #[test]
    fn verify_rejects_empty() {
        assert!(!verify_password("x", "", ""));
        assert!(!verify_password("x", "abc", ""));
    }

    #[test]
    fn policy_accepts_good_password() {
        assert!(validate_password("TestPass123!").is_ok());
    }

    #[test]
    fn policy_rejects_bad_passwords() {
        assert!(validate_password("short1!").is_err()); // < 8
        assert!(validate_password("alllowercase1!").is_err()); // no upper
        assert!(validate_password("ALLUPPERCASE1!").is_err()); // no lower
        assert!(validate_password("NoDigits!!").is_err()); // no digit
        assert!(validate_password("NoSpecial123").is_err()); // no special
        assert!(validate_password(&"Aa1!".repeat(40)).is_err()); // > 128 (160)
    }

    #[test]
    fn email_validation() {
        assert!(validate_email("").is_ok());
        assert!(validate_email("a@b.co").is_ok());
        assert!(validate_email("user@example.com").is_ok());
        assert!(validate_email("nope").is_err());
        assert!(validate_email("a@b").is_err());
        assert!(validate_email("@b.co").is_err());
        assert!(validate_email("a@.co").is_err());
    }
}
