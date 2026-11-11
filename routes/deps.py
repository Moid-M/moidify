import hashlib
import hmac
import secrets
import shutil
import time
import unicodedata
from pathlib import Path
from typing import Optional
from collections import defaultdict

from fastapi import HTTPException, Header
import re
from pydantic import BaseModel

from config import BASE_DIR, MUSIC_DIR, COVERS_DIR
from database import get_connection

FFMPEG_PATH = shutil.which("ffmpeg")

TRANSCODE_MAP = {
    "original": {"codec": None, "bitrate": None},
    "high":     {"codec": "libopus", "bitrate": "192k"},
    "medium":   {"codec": "libopus", "bitrate": "128k"},
    "low":      {"codec": "libopus", "bitrate": "96k"},
    "voice":    {"codec": "libopus", "bitrate": "64k"},
}

try:
    with open(BASE_DIR / "version.txt") as f:
        APP_VERSION = f.read().strip()
except Exception:
    APP_VERSION = "0.0.0"

# Rate limiter (in-memory, single-process)
_rate_limit_store = defaultdict(list)
_RATE_LIMIT_MAX_KEYS = 10000

def check_rate_limit(key: str, max_attempts: int = 10, window_seconds: int = 900):
    """Returns True if allowed, False if rate limited."""
    now = time.time()
    _rate_limit_store[key] = [t for t in _rate_limit_store[key] if t > now - window_seconds]
    if len(_rate_limit_store[key]) >= max_attempts:
        return False
    _rate_limit_store[key].append(now)
    # Prevent unbounded growth from unique keys (e.g. per-username IPs)
    if len(_rate_limit_store) > _RATE_LIMIT_MAX_KEYS:
        empty = [k for k, v in _rate_limit_store.items() if not v]
        for k in empty[: len(empty) // 2]:
            del _rate_limit_store[k]
        if len(_rate_limit_store) > _RATE_LIMIT_MAX_KEYS:
            # Still too many active keys — drop oldest entries wholesale
            for k in list(_rate_limit_store.keys())[: _RATE_LIMIT_MAX_KEYS // 2]:
                del _rate_limit_store[k]
    return True

def _normalize(s):
    nfkd = unicodedata.normalize("NFKD", s)
    return "".join(c for c in nfkd if not unicodedata.category(c).startswith("M"))


PBKDF2_ITERATIONS = 600000

def _hash_password(password: str, salt: Optional[str] = None):
    if salt is None:
        salt = secrets.token_hex(16)
    pwd_hash = hashlib.pbkdf2_hmac(
        "sha256", password.encode(), salt.encode(), PBKDF2_ITERATIONS
    ).hex()
    return pwd_hash, salt


def _verify_password(password: str, salt: str, expected_hash: str) -> bool:
    """Timing-safe PBKDF2 password verification."""
    if not salt or not expected_hash:
        return False
    candidate = hashlib.pbkdf2_hmac(
        "sha256", password.encode(), salt.encode(), PBKDF2_ITERATIONS
    ).hex()
    return hmac.compare_digest(candidate, expected_hash)


# --- Reversible password copy for Subsonic token auth -----------------------
# Subsonic clients send t = md5(password + salt). Verifying that requires the
# plaintext password, which a one-way hash cannot provide. Like Navidrome and
# Ampache, we keep a Fernet-encrypted copy of the password solely for this.
# Web login continues to use PBKDF2; this copy is never sent to clients.

_fernet = None

def _get_fernet():
    """Lazily create/load the password-encryption key (file next to the DB)."""
    global _fernet
    if _fernet is not None:
        return _fernet
    from cryptography.fernet import Fernet
    import config
    key_path = config.DB_PATH.parent / ".moidify_secret_key"
    try:
        if key_path.exists():
            key = key_path.read_bytes().strip()
        else:
            key = Fernet.generate_key()
            config.DB_PATH.parent.mkdir(parents=True, exist_ok=True)
            key_path.write_bytes(key)
            try:
                key_path.chmod(0o600)
            except OSError:
                pass
        _fernet = Fernet(key)
    except Exception:
        _fernet = None
    return _fernet

def encrypt_password_for_subsonic(password: str) -> Optional[str]:
    f = _get_fernet()
    if f is None:
        return None
    try:
        return f.encrypt(password.encode()).decode()
    except Exception:
        return None

def decrypt_password_for_subsonic(token: Optional[str]) -> Optional[str]:
    if not token:
        return None
    f = _get_fernet()
    if f is None:
        return None
    try:
        return f.decrypt(token.encode()).decode()
    except Exception:
        return None

def _store_subsonic_password(conn, user_id: int, password: str):
    """Persist/replaces the encrypted password copy used by Subsonic auth."""
    enc = encrypt_password_for_subsonic(password)
    if enc is None:
        return
    conn.execute(
        "UPDATE users SET password_encrypted = ? WHERE id = ?",
        (enc, user_id),
    )


def _token_hash(token: str) -> str:
    return hashlib.sha256(token.encode()).hexdigest()


def _create_session(user_id: int) -> str:
    token = secrets.token_urlsafe(48)
    expires = int(time.time()) + 2592000
    conn = get_connection()
    conn.execute(
        "INSERT INTO sessions (token, user_id, expires_at, token_hash) VALUES (?, ?, ?, ?)",
        (_token_hash(token), user_id, expires, _token_hash(token)),
    )
    conn.commit()
    conn.close()
    return token


def _get_user_from_token(token: Optional[str]):
    if not token:
        return None
    conn = get_connection()
    conn.execute("DELETE FROM sessions WHERE expires_at IS NOT NULL AND expires_at < ?",
                 (int(time.time()),))
    row = conn.execute(
        """SELECT u.id, u.username, u.email, u.is_admin
           FROM sessions s JOIN users u ON s.user_id = u.id
           WHERE s.token_hash = ? AND (s.expires_at IS NULL OR s.expires_at > ?)""",
        (_token_hash(token), int(time.time())),
    ).fetchone()
    conn.commit()
    conn.close()
    return dict(row) if row else None


def _require_admin(token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if not user:
        raise HTTPException(401, "Not authenticated")
    if not user.get("is_admin"):
        raise HTTPException(403, "Admin access required")
    return user


def _validate_password(pwd: str) -> str:
    if len(pwd) < 8:
        raise HTTPException(400, "Password must be at least 8 characters")
    if len(pwd) > 128:
        raise HTTPException(400, "Password must be at most 128 characters")
    if not re.search(r'[a-z]', pwd):
        raise HTTPException(400, "Password must contain a lowercase letter")
    if not re.search(r'[A-Z]', pwd):
        raise HTTPException(400, "Password must contain an uppercase letter")
    if not re.search(r'[0-9]', pwd):
        raise HTTPException(400, "Password must contain a digit")
    if not re.search(r'[!@#$%^&*(),.?":{}|<>_\-]', pwd):
        raise HTTPException(400, "Password must contain a special character")
    return pwd


def _check_account_locked(username: str):
    conn = get_connection()
    row = conn.execute(
        "SELECT failed_attempts, locked_until FROM users WHERE username = ?",
        (username,),
    ).fetchone()
    conn.close()
    if not row:
        return False
    if row["locked_until"] and time.time() < row["locked_until"]:
        remaining = int(row["locked_until"] - time.time())
        raise HTTPException(429, f"Account locked. Try again in {remaining} seconds.")
    return row["failed_attempts"] if row["failed_attempts"] else 0


def _increment_failed_attempts(username: str):
    conn = get_connection()
    row = conn.execute(
        "SELECT id, failed_attempts FROM users WHERE username = ?", (username,)
    ).fetchone()
    if row:
        attempts = (row["failed_attempts"] or 0) + 1
        if attempts >= 5:
            lock_duration = min(30 * 60 * (2 ** (attempts - 5)), 86400)  # 30m, 1h, 2h, 4h, ... max 24h
            conn.execute(
                "UPDATE users SET failed_attempts = ?, locked_until = ? WHERE id = ?",
                (attempts, int(time.time()) + lock_duration, row["id"]),
            )
        else:
            conn.execute(
                "UPDATE users SET failed_attempts = ? WHERE id = ?",
                (attempts, row["id"]),
            )
        conn.commit()
    conn.close()


def _reset_failed_attempts(username: str):
    conn = get_connection()
    conn.execute(
        "UPDATE users SET failed_attempts = 0, locked_until = NULL WHERE username = ?",
        (username,),
    )
    conn.commit()
    conn.close()


def _is_registration_open() -> bool:
    # Registration is never open before the setup wizard has created an admin.
    conn = get_connection()
    admin_count = conn.execute(
        "SELECT COUNT(*) FROM users WHERE is_admin = 1"
    ).fetchone()[0]
    if admin_count == 0:
        conn.close()
        return False
    row = conn.execute(
        "SELECT value FROM settings WHERE key = 'registration_open'"
    ).fetchone()
    conn.close()
    if row and row["value"] == "0":
        return False
    return True


def _safe_name(s: str) -> str:
    return "".join(c if c.isalnum() or c in " _-" else "_" for c in s).strip()


def _fetch_lyrics_from_lrclib(artist: str, title: str, album: str) -> Optional[str]:
    import json, urllib.request, urllib.parse
    params = urllib.parse.urlencode({
        "artist_name": artist,
        "track_name": title,
        "album_name": album,
    })
    req = urllib.request.Request(
        f"https://lrclib.net/api/get?{params}",
        headers={"User-Agent": "Moidify/1.0"},
    )
    try:
        with urllib.request.urlopen(req, timeout=8) as resp:
            data = json.loads(resp.read().decode())
        return data.get("syncedLyrics") or data.get("plainLyrics") or None
    except Exception:
        return None


# Shared Pydantic models

class RegisterBody(BaseModel):
    username: str
    password: str
    email: Optional[str] = None

    @classmethod
    def validate_email(cls, v):
        if v is not None and v.strip():
            if not re.match(r'^[^@\s]+@[^@\s]+\.[^@\s]+$', v):
                raise ValueError("Invalid email format")
        return v

class LoginBody(BaseModel):
    username: str
    password: str

class CreatePlaylistBody(BaseModel):
    name: str

class AddTrackBody(BaseModel):
    track_id: int

class ReorderPlaylistBody(BaseModel):
    order: list[int]

class CreateFolderBody(BaseModel):
    name: str

class RenameFolderBody(BaseModel):
    name: str

class SetPlaylistFolderBody(BaseModel):
    folder_id: Optional[int] = None

class SetupInitBody(BaseModel):
    username: str
    password: str
    music_dir: Optional[str] = None
    max_upload_size: Optional[int] = None

class ScheduleBody(BaseModel):
    interval_hours: float

class ToggleAdminBody(BaseModel):
    is_admin: bool

class CreateUserBody(BaseModel):
    username: str
    password: str
    is_admin: bool = False

class ChangePasswordBody(BaseModel):
    password: str

class ChangeOwnPasswordBody(BaseModel):
    current_password: str
    new_password: str


class PlayerStateBody(BaseModel):
    queue: list = []
    current_index: int = -1
    current_time: float = 0
    shuffle: bool = False
    repeat_mode: str = "off"
    playback_speed: float = 1.0
    volume: float = 0.7
    shuffle_order: list = []
    shuffle_index: int = 0


class ShareAlbumBody(BaseModel):
    album: str
    artist: Optional[str] = None


class UnshareAlbumBody(BaseModel):
    album: str
    artist: Optional[str] = None


class UpdateLyricsBody(BaseModel):
    lyrics: str = ""
