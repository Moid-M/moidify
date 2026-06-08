import json
import logging
import os
import sqlite3
from pathlib import Path

logger = logging.getLogger(__name__)

BASE_DIR = Path(__file__).parent

PORT = int(os.environ.get("MOIDIFY_PORT", 8000))

INSTALLED_CONFIG = Path("/etc/moidify/config.json")
_cfg_music = None
_cfg_covers = None
_cfg_db = None
_cfg_max_upload = None
if INSTALLED_CONFIG.exists():
    try:
        with open(INSTALLED_CONFIG) as f:
            _cfg = json.load(f)
        _cfg_music = _cfg.get("music_dir")
        _cfg_covers = _cfg.get("covers_dir")
        _cfg_db = _cfg.get("db_path")
        _cfg_max_upload = _cfg.get("max_upload_size")
        PORT = int(_cfg.get("port", PORT))
    except (json.JSONDecodeError, OSError):
        pass

MUSIC_DIR = Path(os.environ.get("MOIDIFY_MUSIC_DIR") or _cfg_music or str(BASE_DIR / "music"))
COVERS_DIR = Path(os.environ.get("MOIDIFY_COVERS_DIR") or _cfg_covers or str(BASE_DIR / "covers"))
DB_PATH = Path(os.environ.get("MOIDIFY_DB_PATH") or _cfg_db or str(BASE_DIR / "data" / "music.db"))
PORT = int(os.environ.get("MOIDIFY_PORT", PORT))

# Default max upload size: 2.5 GB (in bytes)
DEFAULT_MAX_UPLOAD = int(2.5 * 1024 * 1024 * 1024)

# Check database for runtime override (set via setup or admin UI)
_db_max_upload = None
try:
    _d = DB_PATH if DB_PATH.exists() else None
    if _d:
        _c = sqlite3.connect(str(_d), timeout=5)
        row = _c.execute("SELECT value FROM settings WHERE key = 'max_upload_size'").fetchone()
        if row:
            _db_max_upload = int(row[0])
        _c.close()
except Exception:
    pass

MAX_UPLOAD_SIZE = int(os.environ.get("MOIDIFY_MAX_UPLOAD_SIZE") or _db_max_upload or _cfg_max_upload or DEFAULT_MAX_UPLOAD)

os.makedirs(MUSIC_DIR, exist_ok=True)
os.makedirs(COVERS_DIR, exist_ok=True)
os.makedirs(DB_PATH.parent, exist_ok=True)

logger.info("MUSIC_DIR=%s", MUSIC_DIR)
logger.info("COVERS_DIR=%s", COVERS_DIR)
logger.info("DB_PATH=%s", DB_PATH)
logger.info("MAX_UPLOAD_SIZE=%s (%s GB)", MAX_UPLOAD_SIZE, round(MAX_UPLOAD_SIZE / 1024 / 1024 / 1024, 1))

STATIC_DIR = BASE_DIR / "static"

# Last.fm API keys (for scrobbling) — set in /etc/moidify/config.json or env vars
LASTFM_KEY = os.environ.get("MOIDIFY_LASTFM_KEY", "")
LASTFM_SECRET = os.environ.get("MOIDIFY_LASTFM_SECRET", "")


