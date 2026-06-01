import sqlite3
import time
from config import DB_PATH


def get_connection():
    conn = sqlite3.connect(str(DB_PATH), timeout=10)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA journal_mode=WAL")
    conn.execute("PRAGMA foreign_keys=ON")
    conn.execute("PRAGMA synchronous=NORMAL")
    return conn


def init_db():
    conn = get_connection()
    conn.executescript("""
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT UNIQUE NOT NULL,
            email TEXT,
            password_hash TEXT NOT NULL,
            salt TEXT NOT NULL,
            is_admin INTEGER DEFAULT 0,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS sessions (
            token TEXT PRIMARY KEY,
            user_id INTEGER NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
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
            disc_number INTEGER DEFAULT 1,
            genre TEXT,
            year INTEGER,
            duration REAL,
            has_cover INTEGER DEFAULT 0,
            play_count INTEGER DEFAULT 0,
            lyrics TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS playlists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            user_id INTEGER NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
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
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS player_state (
            user_id INTEGER PRIMARY KEY,
            queue TEXT NOT NULL DEFAULT '[]',
            current_index INTEGER NOT NULL DEFAULT -1,
            current_time REAL NOT NULL DEFAULT 0,
            shuffle INTEGER NOT NULL DEFAULT 0,
            repeat_mode TEXT NOT NULL DEFAULT 'off',
            playback_speed REAL NOT NULL DEFAULT 1.0,
            volume REAL NOT NULL DEFAULT 0.7,
            shuffle_order TEXT NOT NULL DEFAULT '[]',
            shuffle_index INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
    """)

    # Indexes for performance
    for idx in [
        "CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist)",
        "CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album)",
        "CREATE INDEX IF NOT EXISTS idx_tracks_title ON tracks(title)",
        "CREATE INDEX IF NOT EXISTS idx_tracks_file_hash ON tracks(file_hash)",
        "CREATE INDEX IF NOT EXISTS idx_play_history_played_at ON play_history(played_at)",
        "CREATE INDEX IF NOT EXISTS idx_playlists_user_id ON playlists(user_id)",
        "CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id)",
        "CREATE INDEX IF NOT EXISTS idx_shared_albums_album ON shared_albums(album)",
        "CREATE INDEX IF NOT EXISTS idx_tracks_cover_hash ON tracks(cover_hash)",
    ]:
        try:
            conn.execute(idx)
        except sqlite3.OperationalError:
            pass

    # migrate existing tables — add columns if missing (safe to run on fresh DB too)
    _existing_cols = {}

    def _has_col(table, col):
        if table not in _existing_cols:
            _existing_cols[table] = {
                r[1] for r in conn.execute(f"PRAGMA table_info({table})").fetchall()
            }
        return col in _existing_cols[table]

    for stmt in [
        "ALTER TABLE users ADD COLUMN is_admin INTEGER DEFAULT 0",
        "ALTER TABLE tracks ADD COLUMN play_count INTEGER DEFAULT 0",
        "ALTER TABLE tracks ADD COLUMN rating INTEGER DEFAULT 0",
        "ALTER TABLE tracks ADD COLUMN disc_number INTEGER DEFAULT 1",
        "ALTER TABLE tracks ADD COLUMN album_artist TEXT DEFAULT NULL",
        "ALTER TABLE tracks ADD COLUMN lyrics TEXT DEFAULT NULL",
        "ALTER TABLE playlists ADD COLUMN folder_id INTEGER REFERENCES playlist_folders(id) ON DELETE SET NULL",
        "ALTER TABLE sessions ADD COLUMN expires_at INTEGER DEFAULT NULL",
        "ALTER TABLE tracks ADD COLUMN cover_hash TEXT DEFAULT NULL",
        "ALTER TABLE tracks ADD COLUMN artist_img_hash TEXT DEFAULT NULL",
    ]:
        table = stmt.split()[2]
        col = stmt.split()[3].split(" ")[0].split("(")[0]
        if not _has_col(table, col):
            try:
                conn.execute(stmt)
            except sqlite3.OperationalError as e:
                if "duplicate column" not in str(e).lower():
                    print(f"Migration warning: {e}")
        else:
            pass  # column already exists

    # index on migrated column
    try:
        conn.execute("CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(expires_at)")
    except sqlite3.OperationalError:
        pass

    # ensure the "admin" user has is_admin=1
    for attempt in range(3):
        try:
            conn.execute("UPDATE users SET is_admin = 1 WHERE username = 'admin'")
            break
        except sqlite3.OperationalError as e:
            if "locked" in str(e).lower() and attempt < 2:
                time.sleep(1)
            else:
                raise
    conn.commit()
    conn.close()
