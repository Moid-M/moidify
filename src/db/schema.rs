use sqlx::SqlitePool;

/// Create tables if missing, then add any columns introduced after v1.
/// Byte-compatible with the Python `database.py` schema so existing
/// `music.db` files work unchanged.
pub async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(SCHEMA).execute(pool).await?;

    // Column-add migrations: check PRAGMA table_info, add if missing.
    let adds: &[(&str, &str, &str)] = &[
        (
            "users",
            "is_admin",
            "ALTER TABLE users ADD COLUMN is_admin INTEGER DEFAULT 0",
        ),
        (
            "users",
            "failed_attempts",
            "ALTER TABLE users ADD COLUMN failed_attempts INTEGER DEFAULT 0",
        ),
        (
            "users",
            "locked_until",
            "ALTER TABLE users ADD COLUMN locked_until INTEGER DEFAULT NULL",
        ),
        (
            "users",
            "password_encrypted",
            "ALTER TABLE users ADD COLUMN password_encrypted TEXT DEFAULT NULL",
        ),
        (
            "tracks",
            "play_count",
            "ALTER TABLE tracks ADD COLUMN play_count INTEGER DEFAULT 0",
        ),
        (
            "tracks",
            "disc_number",
            "ALTER TABLE tracks ADD COLUMN disc_number INTEGER DEFAULT NULL",
        ),
        (
            "tracks",
            "album_artist",
            "ALTER TABLE tracks ADD COLUMN album_artist TEXT DEFAULT NULL",
        ),
        (
            "tracks",
            "lyrics",
            "ALTER TABLE tracks ADD COLUMN lyrics TEXT DEFAULT NULL",
        ),
        (
            "tracks",
            "cover_hash",
            "ALTER TABLE tracks ADD COLUMN cover_hash TEXT DEFAULT NULL",
        ),
        (
            "tracks",
            "artist_img_hash",
            "ALTER TABLE tracks ADD COLUMN artist_img_hash TEXT DEFAULT NULL",
        ),
        (
            "playlists",
            "folder_id",
            "ALTER TABLE playlists ADD COLUMN folder_id INTEGER DEFAULT NULL",
        ),
        (
            "sessions",
            "expires_at",
            "ALTER TABLE sessions ADD COLUMN expires_at INTEGER DEFAULT NULL",
        ),
        (
            "sessions",
            "token_hash",
            "ALTER TABLE sessions ADD COLUMN token_hash TEXT DEFAULT NULL",
        ),
    ];

    for (table, column, stmt) in adds {
        if !has_column(pool, table, column).await? {
            tracing::info!(table, column, "adding column");
            sqlx::query(stmt).execute(pool).await?;
        }
    }

    // Best-effort secondary indexes (ignore failures, same as Python).
    for idx in INDEXES {
        let _ = sqlx::query(idx).execute(pool).await;
    }

    Ok(())
}

async fn has_column(pool: &SqlitePool, table: &str, column: &str) -> anyhow::Result<bool> {
    // PRAGMA table_info returns (cid, name, type, notnull, dflt_value, pk).
    // Identifiers can't be bound as parameters; `table` comes only from the
    // hardcoded migrate list above, so interpolation is safe here.
    let pragma = format!("PRAGMA table_info({table})");
    let rows: Vec<(i64, String)> = sqlx::query_as(&pragma).fetch_all(pool).await?;
    Ok(rows.iter().any(|(_, name)| name == column))
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT UNIQUE NOT NULL,
    email TEXT,
    password_hash TEXT NOT NULL,
    salt TEXT NOT NULL,
    password_encrypted TEXT DEFAULT NULL,
    is_admin INTEGER DEFAULT 0,
    failed_attempts INTEGER DEFAULT 0,
    locked_until INTEGER DEFAULT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS sessions (
    token TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    expires_at INTEGER DEFAULT NULL,
    token_hash TEXT DEFAULT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS tracks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT UNIQUE NOT NULL,
    file_hash TEXT,
    title TEXT,
    artist TEXT,
    album_artist TEXT,
    album TEXT,
    track_number INTEGER,
    disc_number INTEGER,
    genre TEXT,
    year INTEGER,
    duration REAL,
    has_cover INTEGER DEFAULT 0,
    play_count INTEGER DEFAULT 0,
    lyrics TEXT,
    cover_hash TEXT,
    artist_img_hash TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS playlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    user_id INTEGER NOT NULL,
    folder_id INTEGER DEFAULT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (folder_id) REFERENCES playlist_folders(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS playlist_tracks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    playlist_id INTEGER NOT NULL,
    track_id INTEGER NOT NULL,
    position INTEGER NOT NULL,
    added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS favorites (
    user_id INTEGER NOT NULL,
    track_id INTEGER NOT NULL,
    added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, track_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS play_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id INTEGER NOT NULL,
    played_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS shared_playlists (
    token TEXT PRIMARY KEY,
    playlist_id INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (playlist_id) REFERENCES playlists(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS playlist_folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    user_id INTEGER NOT NULL,
    sort_order INTEGER DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS shared_albums (
    token TEXT PRIMARY KEY,
    album TEXT NOT NULL,
    artist TEXT,
    user_id INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT
);

CREATE TABLE IF NOT EXISTS player_state (
    user_id INTEGER PRIMARY KEY,
    queue TEXT,
    current_index INTEGER DEFAULT 0,
    current_time REAL DEFAULT 0,
    shuffle INTEGER DEFAULT 0,
    repeat_mode TEXT DEFAULT 'off',
    playback_speed REAL DEFAULT 1.0,
    volume REAL DEFAULT 0.7,
    shuffle_order TEXT,
    shuffle_index INTEGER DEFAULT 0,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS lastfm_settings (
    user_id INTEGER PRIMARY KEY,
    lastfm_username TEXT,
    session_key TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
"#;

const INDEXES: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist)",
    "CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album)",
    "CREATE INDEX IF NOT EXISTS idx_tracks_title ON tracks(title)",
    "CREATE INDEX IF NOT EXISTS idx_tracks_file_hash ON tracks(file_hash)",
    "CREATE INDEX IF NOT EXISTS idx_tracks_cover_hash ON tracks(cover_hash)",
    "CREATE INDEX IF NOT EXISTS idx_play_history_played_at ON play_history(played_at)",
    "CREATE INDEX IF NOT EXISTS idx_playlists_user_id ON playlists(user_id)",
    "CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id)",
    "CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(expires_at)",
    "CREATE INDEX IF NOT EXISTS idx_sessions_token_hash ON sessions(token_hash)",
    "CREATE INDEX IF NOT EXISTS idx_shared_albums_album ON shared_albums(album)",
];
