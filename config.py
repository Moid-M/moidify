import json
import os
from pathlib import Path

BASE_DIR = Path(__file__).parent

PORT = int(os.environ.get("MOIDIFY_PORT", 8000))

INSTALLED_CONFIG = Path("/etc/moidify/config.json")
_cfg_music = None
_cfg_covers = None
_cfg_db = None
if INSTALLED_CONFIG.exists():
    try:
        with open(INSTALLED_CONFIG) as f:
            _cfg = json.load(f)
        _cfg_music = _cfg.get("music_dir")
        _cfg_covers = _cfg.get("covers_dir")
        _cfg_db = _cfg.get("db_path")
        PORT = int(_cfg.get("port", PORT))
    except (json.JSONDecodeError, OSError):
        pass

MUSIC_DIR = Path(os.environ.get("MOIDIFY_MUSIC_DIR") or _cfg_music or str(BASE_DIR / "music"))
COVERS_DIR = Path(os.environ.get("MOIDIFY_COVERS_DIR") or _cfg_covers or str(BASE_DIR / "covers"))
DB_PATH = Path(os.environ.get("MOIDIFY_DB_PATH") or _cfg_db or str(BASE_DIR / "data" / "music.db"))
PORT = int(os.environ.get("MOIDIFY_PORT", PORT))

os.makedirs(MUSIC_DIR, exist_ok=True)
os.makedirs(COVERS_DIR, exist_ok=True)
os.makedirs(DB_PATH.parent, exist_ok=True)

print(f"[config] MUSIC_DIR={MUSIC_DIR}", flush=True)
print(f"[config] COVERS_DIR={COVERS_DIR}", flush=True)
print(f"[config] DB_PATH={DB_PATH}", flush=True)

STATIC_DIR = BASE_DIR / "static"


