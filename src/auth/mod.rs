//! Authentication primitives (password hashing, sessions, Fernet, extractors).

pub mod extractors;
pub mod fernet;
pub mod password;
pub mod ratelimit;
pub mod session;

use crate::error::AppError;
use fernet::Fernet;
use sqlx::SqlitePool;
use std::path::Path;
use std::sync::Arc;

/// Load (or create) the Fernet key stored next to the database —
/// same file layout as Python: `<db_dir>/.moidify_secret_key`, mode 0600.
pub fn load_or_create_fernet(db_path: &Path) -> anyhow::Result<Fernet> {
    let dir = db_path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let key_path = dir.join(".moidify_secret_key");

    let key_bytes = if key_path.exists() {
        let raw = std::fs::read(&key_path)?;
        let text = String::from_utf8_lossy(&raw);
        text.trim().as_bytes().to_vec()
    } else {
        let key = Fernet::generate_key();
        std::fs::write(&key_path, &key)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600));
        }
        key
    };

    Ok(Fernet::new(&key_bytes)?)
}

/// Store/replaces the Fernet-encrypted password copy used by Subsonic token auth.
pub async fn store_subsonic_password(
    pool: &SqlitePool,
    fernet: &Fernet,
    user_id: i64,
    password: &str,
) -> Result<(), AppError> {
    let enc = fernet.encrypt(password.as_bytes());
    let enc = String::from_utf8(enc).map_err(|e| AppError::Internal(e.into()))?;
    sqlx::query("UPDATE users SET password_encrypted = ? WHERE id = ?")
        .bind(&enc)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(())
}

/// Is registration currently open? (Never open before setup creates an admin.)
pub async fn is_registration_open(pool: &SqlitePool) -> Result<bool, AppError> {
    let (admin_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE is_admin = 1")
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    if admin_count == 0 {
        return Ok(false);
    }
    let row: Option<(String,)> =
        sqlx::query_as("SELECT value FROM settings WHERE key = 'registration_open'")
            .fetch_optional(pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    Ok(match row {
        Some((v,)) => v != "0",
        None => true,
    })
}

/// Load the shared Fernet into an `Arc` for AppState.
pub fn shared_fernet(db_path: &Path) -> anyhow::Result<Arc<Fernet>> {
    Ok(Arc::new(load_or_create_fernet(db_path)?))
}
