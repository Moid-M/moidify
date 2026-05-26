import json
import os
from pathlib import Path

BASE_DIR = Path(__file__).parent

INSTALLED_CONFIG = Path("/etc/moidify/config.json")
if INSTALLED_CONFIG.exists():
    with open(INSTALLED_CONFIG) as f:
        _cfg = json.load(f)
    MUSIC_DIR = Path(_cfg.get("music_dir", "/var/lib/moidify/music"))
    COVERS_DIR = Path(_cfg.get("covers_dir", "/var/lib/moidify/covers"))
    DB_PATH = Path(_cfg.get("db_path", "/var/lib/moidify/music.db"))
else:
    MUSIC_DIR = BASE_DIR / "music"
    COVERS_DIR = BASE_DIR / "covers"
    DB_PATH = BASE_DIR / "data" / "music.db"

MUSIC_DIR = Path(os.environ.get("MOIDIFY_MUSIC_DIR", str(MUSIC_DIR)))
COVERS_DIR = Path(os.environ.get("MOIDIFY_COVERS_DIR", str(COVERS_DIR)))
DB_PATH = Path(os.environ.get("MOIDIFY_DB_PATH", str(DB_PATH)))

os.makedirs(MUSIC_DIR, exist_ok=True)
os.makedirs(COVERS_DIR, exist_ok=True)
os.makedirs(DB_PATH.parent, exist_ok=True)


