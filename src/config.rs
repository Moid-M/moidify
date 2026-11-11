use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Runtime configuration.
///
/// Precedence (matches Python `config.py`):
///   environment variable  >  /etc/moidify/config.json  >  defaults
/// `max_upload_size` additionally honors the DB `settings` table (checked at runtime).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub port: u16,
    pub music_dir: PathBuf,
    pub covers_dir: PathBuf,
    pub db_path: PathBuf,
    pub max_upload_size: i64,
    pub lastfm_api_key: String,
    pub lastfm_api_secret: String,
}

impl Default for Config {
    fn default() -> Self {
        let base = base_dir();
        Self {
            port: 8000,
            music_dir: base.join("music"),
            covers_dir: base.join("covers"),
            db_path: base.join("data").join("music.db"),
            max_upload_size: (2.5_f64 * 1024_f64.powi(3)) as i64, // 2.5 GiB
            lastfm_api_key: String::new(),
            lastfm_api_secret: String::new(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    port: Option<u16>,
    music_dir: Option<PathBuf>,
    covers_dir: Option<PathBuf>,
    db_path: Option<PathBuf>,
    max_upload_size: Option<i64>,
    lastfm_api_key: Option<String>,
    lastfm_api_secret: Option<String>,
}

fn base_dir() -> PathBuf {
    // Repo root: <repo>/src/config.rs → parent of src/
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

impl Config {
    /// Load config.json (if present) then apply env overrides.
    pub fn load() -> Self {
        let mut cfg = Self::default();

        let installed = Path::new("/etc/moidify/config.json");
        let path = if installed.exists() {
            installed.to_path_buf()
        } else {
            base_dir().join("config.json")
        };

        if let Ok(raw) = std::fs::read_to_string(&path) {
            match serde_json::from_str::<FileConfig>(&raw) {
                Ok(fc) => {
                    if let Some(v) = fc.port {
                        cfg.port = v;
                    }
                    if let Some(v) = fc.music_dir {
                        cfg.music_dir = v;
                    }
                    if let Some(v) = fc.covers_dir {
                        cfg.covers_dir = v;
                    }
                    if let Some(v) = fc.db_path {
                        cfg.db_path = v;
                    }
                    if let Some(v) = fc.max_upload_size {
                        cfg.max_upload_size = v;
                    }
                    if let Some(v) = fc.lastfm_api_key {
                        cfg.lastfm_api_key = v;
                    }
                    if let Some(v) = fc.lastfm_api_secret {
                        cfg.lastfm_api_secret = v;
                    }
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "failed to parse config.json")
                }
            }
        }

        // Env always wins (matches Python behavior).
        if let Ok(v) = std::env::var("MOIDIFY_PORT") {
            if let Ok(n) = v.parse() {
                cfg.port = n;
            }
        }
        if let Ok(v) = std::env::var("MOIDIFY_MUSIC_DIR") {
            cfg.music_dir = PathBuf::from(v);
        }
        if let Ok(v) = std::env::var("MOIDIFY_COVERS_DIR") {
            cfg.covers_dir = PathBuf::from(v);
        }
        if let Ok(v) = std::env::var("MOIDIFY_DB_PATH") {
            cfg.db_path = PathBuf::from(v);
        }
        if let Ok(v) = std::env::var("MOIDIFY_MAX_UPLOAD_SIZE") {
            if let Ok(n) = v.parse() {
                cfg.max_upload_size = n;
            }
        }
        if let Ok(v) = std::env::var("MOIDIFY_LASTFM_KEY") {
            cfg.lastfm_api_key = v;
        }
        if let Ok(v) = std::env::var("MOIDIFY_LASTFM_SECRET") {
            cfg.lastfm_api_secret = v;
        }

        cfg
    }

    /// Create the directories the server needs (mirrors Python `config.py` side effects).
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.music_dir)?;
        std::fs::create_dir_all(&self.covers_dir)?;
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }

    /// App version from version.txt (fallback "3.0.0").
    pub fn version() -> String {
        let path = base_dir().join("version.txt");
        std::fs::read_to_string(path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "3.0.0".to_string())
    }
}
