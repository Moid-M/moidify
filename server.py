import hashlib
import os
import secrets
import shutil
import subprocess
import tempfile
import unicodedata
import zipfile
from contextlib import asynccontextmanager
from pathlib import Path
from typing import Optional

import uvicorn
from fastapi import FastAPI, BackgroundTasks, HTTPException, Header, Query, UploadFile, File
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import FileResponse, StreamingResponse
from fastapi.staticfiles import StaticFiles
from pydantic import BaseModel

from config import BASE_DIR, MUSIC_DIR, COVERS_DIR, PORT
from database import get_connection, init_db
from scanner import scan_existing, start_watcher, process_file, get_scan_status

STATIC_DIR = BASE_DIR / "static"


@asynccontextmanager
async def lifespan(app: FastAPI):
    init_db()
    scan_existing()
    start_watcher()
    yield


app = FastAPI(title="Moidify", lifespan=lifespan)

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["*"],
    allow_headers=["*"],
)


# ---------------------------------------------------------------------------
# Search helpers
# ---------------------------------------------------------------------------

def _normalize(s):
    nfkd = unicodedata.normalize("NFKD", s)
    return "".join(c for c in nfkd if not unicodedata.category(c).startswith("M"))


# ---------------------------------------------------------------------------
# Auth helpers
# ---------------------------------------------------------------------------

def _hash_password(password: str, salt: Optional[str] = None):
    if salt is None:
        salt = secrets.token_hex(16)
    pwd_hash = hashlib.pbkdf2_hmac(
        "sha256", password.encode(), salt.encode(), 100000
    ).hex()
    return pwd_hash, salt


def _create_session(user_id: int) -> str:
    token = secrets.token_urlsafe(48)
    conn = get_connection()
    conn.execute("DELETE FROM sessions WHERE user_id = ?", (user_id,))
    conn.execute(
        "INSERT INTO sessions (token, user_id) VALUES (?, ?)", (token, user_id)
    )
    conn.commit()
    conn.close()
    return token


def _get_user_from_token(token: Optional[str]):
    if not token:
        return None
    conn = get_connection()
    row = conn.execute(
        """SELECT u.id, u.username, u.email, u.is_admin
           FROM sessions s JOIN users u ON s.user_id = u.id
           WHERE s.token = ?""",
        (token,),
    ).fetchone()
    conn.close()
    return dict(row) if row else None

def _require_admin(token: Optional[str]):
    user = _get_user_from_token(token)
    if not user:
        raise HTTPException(401, "Not authenticated")
    if not user.get("is_admin"):
        raise HTTPException(403, "Admin access required")
    return user


# ---------------------------------------------------------------------------
# Auth endpoints
# ---------------------------------------------------------------------------

class RegisterBody(BaseModel):
    username: str
    password: str
    email: Optional[str] = None


class LoginBody(BaseModel):
    username: str
    password: str


@app.post("/api/auth/register")
def register(body: RegisterBody):
    if len(body.password) < 3:
        raise HTTPException(400, "Password must be at least 3 characters")
    if len(body.username) < 1:
        raise HTTPException(400, "Username is required")
    conn = get_connection()
    existing = conn.execute(
        "SELECT id FROM users WHERE username = ?", (body.username,)
    ).fetchone()
    if existing:
        conn.close()
        raise HTTPException(400, "Username already taken")
    pwd_hash, salt = _hash_password(body.password)
    conn.execute(
        "INSERT INTO users (username, email, password_hash, salt) VALUES (?, ?, ?, ?)",
        (body.username, body.email, pwd_hash, salt),
    )
    conn.commit()
    conn.close()
    return {"ok": True}


@app.post("/api/auth/login")
def login(body: LoginBody):
    conn = get_connection()
    row = conn.execute(
        "SELECT * FROM users WHERE username = ?", (body.username,)
    ).fetchone()
    conn.close()
    if row is None or not row["salt"]:
        raise HTTPException(401, "Invalid credentials")
    pwd_hash, _ = _hash_password(body.password, row["salt"])
    if pwd_hash != row["password_hash"]:
        raise HTTPException(401, "Invalid credentials")
    token = _create_session(row["id"])
    return {
        "token": token,
        "user": {"id": row["id"], "username": row["username"], "email": row["email"]},
    }


@app.get("/api/auth/me")
def me(token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Not logged in")
    return user


# ---------------------------------------------------------------------------
# Transcoding helpers
# ---------------------------------------------------------------------------

FFMPEG_PATH = shutil.which("ffmpeg")

TRANSCODE_MAP = {
    "high":  {"codec": None, "bitrate": None},         # original
    "medium": {"codec": "libmp3lame", "bitrate": "256k"},
    "low":   {"codec": "libmp3lame", "bitrate": "128k"},
    "voice": {"codec": "libopus", "bitrate": "64k"},
}


def _transcode_stream(file_path: str, quality: str):
    if not FFMPEG_PATH or quality == "high" or quality not in TRANSCODE_MAP:
        return None
    cfg = TRANSCODE_MAP[quality]
    args = [
        FFMPEG_PATH, "-i", file_path, "-f", "mp3",
        "-acodec", cfg["codec"], "-ab", cfg["bitrate"],
        "-vn", "-nostdin", "-loglevel", "error", "-",
    ]
    if cfg["codec"] == "libopus":
        args[args.index("-f") + 1] = "ogg"
        args[args.index("-acodec") + 1] = "libopus"
    process = subprocess.Popen(
        args, stdout=subprocess.PIPE, stderr=subprocess.PIPE
    )
    return process


# ---------------------------------------------------------------------------
# Track endpoints
# ---------------------------------------------------------------------------

@app.get("/api/tracks")
def list_tracks(search: Optional[str] = None):
    conn = get_connection()
    if search:
        normalized = _normalize(search)
        if normalized.lower() != search.lower():
            rows = conn.execute(
                """SELECT DISTINCT * FROM tracks
                   WHERE title LIKE ? OR artist LIKE ? OR album LIKE ?
                      OR title LIKE ? OR artist LIKE ? OR album LIKE ?
                   ORDER BY artist, album, track_number""",
                (f"%{search}%", f"%{search}%", f"%{search}%",
                 f"%{normalized}%", f"%{normalized}%", f"%{normalized}%"),
            ).fetchall()
        else:
            rows = conn.execute(
                """SELECT * FROM tracks
                   WHERE title LIKE ? OR artist LIKE ? OR album LIKE ?
                   ORDER BY artist, album, track_number""",
                (f"%{search}%", f"%{search}%", f"%{search}%"),
            ).fetchall()
    else:
        rows = conn.execute(
            "SELECT * FROM tracks ORDER BY artist, album, track_number"
        ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


@app.get("/api/tracks/{track_id}")
def get_track(track_id: int):
    conn = get_connection()
    row = conn.execute("SELECT * FROM tracks WHERE id = ?", (track_id,)).fetchone()
    conn.close()
    if row is None:
        raise HTTPException(404, "Track not found")
    return dict(row)


@app.get("/api/stream/{track_id}")
def stream_track(track_id: int, quality: Optional[str] = Query("high")):
    conn = get_connection()
    row = conn.execute("SELECT file_path FROM tracks WHERE id = ?", (track_id,)).fetchone()
    conn.close()
    if row is None:
        raise HTTPException(404, "Track not found")

    path = Path(row["file_path"])
    if not path.exists():
        raise HTTPException(404, "File not found on disk")

    proc = _transcode_stream(str(path), quality)
    if proc:
        media_type = "audio/ogg" if quality == "voice" else "audio/mpeg"
        return StreamingResponse(
            proc.stdout,
            media_type=media_type,
            headers={
                "Content-Disposition": "inline",
                "Cache-Control": "no-cache",
                "X-Transcoded": quality,
            },
        )

    ext = path.suffix.lower()
    media_type = {
        ".mp3": "audio/mpeg",
        ".flac": "audio/flac",
        ".ogg": "audio/ogg",
        ".m4a": "audio/mp4",
        ".wav": "audio/wav",
        ".aac": "audio/aac",
        ".wma": "audio/x-ms-wma",
    }.get(ext, "audio/mpeg")

    return FileResponse(str(path), media_type=media_type)


@app.get("/api/cover/{track_id}")
def get_cover(track_id: int):
    for ext in (".jpg", ".png", ".webp"):
        p = COVERS_DIR / f"{track_id}{ext}"
        if p.exists():
            mt = {"jpg": "image/jpeg", "png": "image/png", "webp": "image/webp"}.get(ext[1:], "image/jpeg")
            return FileResponse(str(p), media_type=mt, headers={"Cache-Control": "public, max-age=86400"})
    return FileResponse(
        str(STATIC_DIR / "placeholder-cover.svg"),
        media_type="image/svg+xml",
        headers={"Cache-Control": "public, max-age=3600"},
    )


# ---------------------------------------------------------------------------
# Album / Artist endpoints
# ---------------------------------------------------------------------------

@app.get("/api/albums")
def list_albums():
    conn = get_connection()
    rows = conn.execute(
        """SELECT album, artist, COUNT(*) as track_count,
                  MAX(has_cover) as has_cover, MIN(id) as cover_track_id
           FROM tracks
           WHERE album IS NOT NULL
           GROUP BY album, artist
           ORDER BY artist, album""",
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


@app.get("/api/artists")
def list_artists():
    conn = get_connection()
    rows = conn.execute(
        """SELECT artist, COUNT(*) as track_count,
                  COUNT(DISTINCT album) as album_count
           FROM tracks
           WHERE artist IS NOT NULL
           GROUP BY artist
           ORDER BY artist""",
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


@app.get("/api/albums/tracks")
def list_album_tracks(album: str = Query(...), artist: Optional[str] = Query(None)):
    conn = get_connection()
    if artist:
        rows = conn.execute(
            "SELECT * FROM tracks WHERE album = ? AND artist = ? ORDER BY track_number",
            (album, artist),
        ).fetchall()
    else:
        rows = conn.execute(
            "SELECT * FROM tracks WHERE album = ? ORDER BY track_number", (album,)
        ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


@app.get("/api/artists/tracks")
def list_artist_tracks(artist: str = Query(...)):
    conn = get_connection()
    rows = conn.execute(
        "SELECT * FROM tracks WHERE artist = ? ORDER BY album, track_number", (artist,)
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


@app.get("/api/genres")
def list_genres():
    conn = get_connection()
    rows = conn.execute(
        """SELECT genre, COUNT(*) as track_count,
                  COUNT(DISTINCT album) as album_count
           FROM tracks
           WHERE genre IS NOT NULL AND genre != ''
           GROUP BY genre
           ORDER BY genre""",
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


@app.get("/api/genres/tracks")
def list_genre_tracks(genre: str = Query(...)):
    conn = get_connection()
    rows = conn.execute(
        "SELECT * FROM tracks WHERE genre = ? ORDER BY album, track_number", (genre,)
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


# ---------------------------------------------------------------------------
# Playlist endpoints
# ---------------------------------------------------------------------------

class CreatePlaylistBody(BaseModel):
    name: str


class AddTrackBody(BaseModel):
    track_id: int


class ReorderPlaylistBody(BaseModel):
    order: list[int]


@app.get("/api/playlists")
def list_user_playlists(token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if user is None:
        return []
    conn = get_connection()
    rows = conn.execute(
        """SELECT p.*, COUNT(pt.id) as track_count
           FROM playlists p
           LEFT JOIN playlist_tracks pt ON p.id = pt.playlist_id
           WHERE p.user_id = ?
           GROUP BY p.id
           ORDER BY p.name""",
        (user["id"],),
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


@app.post("/api/playlists")
def create_playlist(body: CreatePlaylistBody, token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Login required")
    conn = get_connection()
    conn.execute(
        "INSERT INTO playlists (name, user_id) VALUES (?, ?)",
        (body.name, user["id"]),
    )
    conn.commit()
    pid = conn.execute("SELECT last_insert_rowid()").fetchone()[0]
    conn.close()
    return {"id": pid, "name": body.name}


@app.delete("/api/playlists/{playlist_id}")
def delete_playlist(playlist_id: int, token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Login required")
    conn = get_connection()
    pl = conn.execute(
        "SELECT * FROM playlists WHERE id = ? AND user_id = ?",
        (playlist_id, user["id"]),
    ).fetchone()
    if pl is None:
        conn.close()
        raise HTTPException(404, "Playlist not found")
    conn.execute("DELETE FROM playlist_tracks WHERE playlist_id = ?", (playlist_id,))
    conn.execute("DELETE FROM playlists WHERE id = ?", (playlist_id,))
    conn.commit()
    conn.close()
    return {"ok": True}


@app.get("/api/playlists/{playlist_id}/tracks")
def get_playlist_tracks(playlist_id: int):
    conn = get_connection()
    rows = conn.execute(
        """SELECT t.*, pt.position, pt.added_at
           FROM playlist_tracks pt
           JOIN tracks t ON pt.track_id = t.id
           WHERE pt.playlist_id = ?
           ORDER BY pt.position""",
        (playlist_id,),
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


@app.post("/api/playlists/{playlist_id}/tracks")
def add_to_playlist(
    playlist_id: int,
    body: AddTrackBody,
    token: Optional[str] = Header(None),
):
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Login required")
    conn = get_connection()
    pl = conn.execute(
        "SELECT * FROM playlists WHERE id = ? AND user_id = ?",
        (playlist_id, user["id"]),
    ).fetchone()
    if pl is None:
        conn.close()
        raise HTTPException(404, "Playlist not found")
    max_pos = conn.execute(
        "SELECT MAX(position) FROM playlist_tracks WHERE playlist_id = ?",
        (playlist_id,),
    ).fetchone()[0]
    pos = (max_pos or 0) + 1
    conn.execute(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)",
        (playlist_id, body.track_id, pos),
    )
    conn.commit()
    conn.close()
    return {"ok": True}


@app.delete("/api/playlists/{playlist_id}/tracks/{track_id}")
def remove_from_playlist(
    playlist_id: int,
    track_id: int,
    token: Optional[str] = Header(None),
):
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Login required")
    conn = get_connection()
    pl = conn.execute(
        "SELECT * FROM playlists WHERE id = ? AND user_id = ?",
        (playlist_id, user["id"]),
    ).fetchone()
    if pl is None:
        conn.close()
        raise HTTPException(404, "Playlist not found")
    conn.execute(
        "DELETE FROM playlist_tracks WHERE playlist_id = ? AND track_id = ?",
        (playlist_id, track_id),
    )
    conn.commit()
    conn.close()
    return {"ok": True}


@app.put("/api/playlists/{playlist_id}/tracks/reorder")
def reorder_playlist_tracks(
    playlist_id: int,
    body: ReorderPlaylistBody,
    token: Optional[str] = Header(None),
):
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Login required")
    conn = get_connection()
    pl = conn.execute(
        "SELECT * FROM playlists WHERE id = ? AND user_id = ?",
        (playlist_id, user["id"]),
    ).fetchone()
    if pl is None:
        conn.close()
        raise HTTPException(404, "Playlist not found")
    order = body.order
    for i, track_id in enumerate(order):
        conn.execute(
            "UPDATE playlist_tracks SET position = ? WHERE playlist_id = ? AND track_id = ?",
            (i + 1, playlist_id, track_id),
        )
    conn.commit()
    conn.close()
    return {"ok": True}


# ---------------------------------------------------------------------------
# Shareable playlist endpoints
# ---------------------------------------------------------------------------

@app.post("/api/playlists/{playlist_id}/share")
def share_playlist(playlist_id: int, token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Login required")
    conn = get_connection()
    pl = conn.execute(
        "SELECT * FROM playlists WHERE id = ? AND user_id = ?",
        (playlist_id, user["id"]),
    ).fetchone()
    if pl is None:
        conn.close()
        raise HTTPException(404, "Playlist not found")
    existing = conn.execute(
        "SELECT token FROM shared_playlists WHERE playlist_id = ?", (playlist_id,)
    ).fetchone()
    if existing:
        conn.close()
        return {"token": existing["token"]}
    share_token = secrets.token_urlsafe(16)
    conn.execute(
        "INSERT INTO shared_playlists (token, playlist_id) VALUES (?, ?)",
        (share_token, playlist_id),
    )
    conn.commit()
    conn.close()
    return {"token": share_token}


@app.get("/api/playlists/{playlist_id}/share")
def get_share_status(playlist_id: int, token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Login required")
    conn = get_connection()
    pl = conn.execute(
        "SELECT * FROM playlists WHERE id = ? AND user_id = ?",
        (playlist_id, user["id"]),
    ).fetchone()
    if pl is None:
        conn.close()
        raise HTTPException(404, "Playlist not found")
    row = conn.execute(
        "SELECT token FROM shared_playlists WHERE playlist_id = ?", (playlist_id,)
    ).fetchone()
    conn.close()
    return {"shared": row is not None, "token": row["token"] if row else None}


@app.delete("/api/playlists/{playlist_id}/share")
def unshare_playlist(playlist_id: int, token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Login required")
    conn = get_connection()
    pl = conn.execute(
        "SELECT * FROM playlists WHERE id = ? AND user_id = ?",
        (playlist_id, user["id"]),
    ).fetchone()
    if pl is None:
        conn.close()
        raise HTTPException(404, "Playlist not found")
    conn.execute("DELETE FROM shared_playlists WHERE playlist_id = ?", (playlist_id,))
    conn.commit()
    conn.close()
    return {"ok": True}


@app.get("/api/shared/{token}")
def get_shared_playlist(token: str):
    conn = get_connection()
    row = conn.execute(
        """SELECT p.id, p.name, p.user_id, u.username
           FROM shared_playlists sp
           JOIN playlists p ON sp.playlist_id = p.id
           JOIN users u ON p.user_id = u.id
           WHERE sp.token = ?""",
        (token,),
    ).fetchone()
    if not row:
        conn.close()
        raise HTTPException(404, "Shared playlist not found")
    tracks = conn.execute(
        """SELECT t.*, pt.position
           FROM playlist_tracks pt
           JOIN tracks t ON pt.track_id = t.id
           WHERE pt.playlist_id = ?
           ORDER BY pt.position""",
        (row["id"],),
    ).fetchall()
    conn.close()
    return {
        "name": row["name"],
        "username": row["username"],
        "tracks": [dict(r) for r in tracks],
    }


# ---------------------------------------------------------------------------
# Favourites endpoints
# ---------------------------------------------------------------------------

@app.get("/api/favorites")
def get_favorites(token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Login required")
    conn = get_connection()
    rows = conn.execute(
        """SELECT t.*, f.added_at
           FROM favorites f
           JOIN tracks t ON f.track_id = t.id
           WHERE f.user_id = ?
           ORDER BY f.added_at DESC""",
        (user["id"],),
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


@app.post("/api/favorites/{track_id}")
def add_favorite(track_id: int, token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Login required")
    conn = get_connection()
    conn.execute(
        "INSERT OR IGNORE INTO favorites (user_id, track_id) VALUES (?, ?)",
        (user["id"], track_id),
    )
    conn.commit()
    conn.close()
    return {"ok": True}


@app.delete("/api/favorites/{track_id}")
def remove_favorite(track_id: int, token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Login required")
    conn = get_connection()
    conn.execute(
        "DELETE FROM favorites WHERE user_id = ? AND track_id = ?",
        (user["id"], track_id),
    )
    conn.commit()
    conn.close()
    return {"ok": True}


@app.post("/api/play/{track_id}")
def increment_play_count(track_id: int):
    conn = get_connection()
    conn.execute("UPDATE tracks SET play_count = COALESCE(play_count,0) + 1 WHERE id = ?", (track_id,))
    conn.execute("INSERT INTO play_history (track_id) VALUES (?)", (track_id,))
    conn.commit()
    conn.close()
    return {"ok": True}

@app.get("/api/favorites/check/{track_id}")
def check_favorite(track_id: int, token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if user is None:
        return {"favorite": False}
    conn = get_connection()
    row = conn.execute(
        "SELECT 1 FROM favorites WHERE user_id = ? AND track_id = ?",
        (user["id"], track_id),
    ).fetchone()
    conn.close()
    return {"favorite": row is not None}


# ---------------------------------------------------------------------------
# Download endpoints
# ---------------------------------------------------------------------------

@app.get("/api/download/album")
def download_album(album: str = Query(...), artist: Optional[str] = Query(None), background_tasks: BackgroundTasks = BackgroundTasks()):
    conn = get_connection()
    if artist:
        rows = conn.execute(
            "SELECT * FROM tracks WHERE album = ? AND artist = ? ORDER BY track_number",
            (album, artist),
        ).fetchall()
    else:
        rows = conn.execute(
            "SELECT * FROM tracks WHERE album = ? ORDER BY track_number", (album,)
        ).fetchall()
    conn.close()

    if not rows:
        raise HTTPException(404, "No tracks found for this album")

    suffix = f"_{artist}" if artist else ""
    safe_name = "".join(c if c.isalnum() or c in " _-" else "_" for c in album + suffix)
    zip_name = safe_name.strip() + ".zip"

    tmp = tempfile.NamedTemporaryFile(delete=False, suffix=".zip")
    with zipfile.ZipFile(tmp, "w", zipfile.ZIP_DEFLATED) as zf:
        for row in rows:
            path = Path(row["file_path"])
            if path.exists():
                track_name = f"{row['track_number'] or 0:02d} - {row['title'] or path.stem}{path.suffix}"
                zf.write(str(path), track_name)

    background_tasks.add_task(os.unlink, tmp.name)
    return FileResponse(
        tmp.name,
        media_type="application/zip",
        filename=zip_name,
        headers={"Content-Disposition": f'attachment; filename="{zip_name}"'},
    )


# ---------------------------------------------------------------------------
# Startup & static serving

# ---------------------------------------------------------------------------
# Setup wizard endpoints
# ---------------------------------------------------------------------------

@app.get("/api/setup/status")
def setup_status():
    conn = get_connection()
    admin_count = conn.execute("SELECT COUNT(*) FROM users WHERE is_admin = 1").fetchone()[0]
    conn.close()
    return {
        "setup_needed": admin_count == 0,
        "has_admin": admin_count > 0,
    }


class SetupInitBody(BaseModel):
    username: str
    password: str
    music_dir: Optional[str] = None


@app.post("/api/setup/init")
def setup_init(body: SetupInitBody):
    conn = get_connection()
    admin_count = conn.execute("SELECT COUNT(*) FROM users WHERE is_admin = 1").fetchone()[0]
    if admin_count > 0:
        conn.close()
        raise HTTPException(400, "Setup already completed")
    existing = conn.execute("SELECT id FROM users WHERE username = ?", (body.username,)).fetchone()
    if existing:
        conn.close()
        raise HTTPException(400, "Username already taken")
    pwd_hash, salt = _hash_password(body.password)
    cursor = conn.execute(
        "INSERT INTO users (username, password_hash, salt, is_admin) VALUES (?, ?, ?, 1)",
        (body.username, pwd_hash, salt),
    )
    user_id = cursor.lastrowid
    conn.commit()
    conn.close()
    token = _create_session(user_id)
    return {"token": token, "user": {"id": user_id, "username": body.username}}


@app.get("/setup")
def setup_page():
    return FileResponse(str(STATIC_DIR / "setup.html"))


app.mount("/static", StaticFiles(directory=str(STATIC_DIR)), name="static")
# ---------------------------------------------------------------------------

# ---- Admin endpoints ----
@app.get("/api/admin/stats")
def admin_stats(token: Optional[str] = Header(None)):
    _require_admin(token)
    conn = get_connection()
    tracks = conn.execute("SELECT COUNT(*) FROM tracks").fetchone()[0]
    artists = conn.execute("SELECT COUNT(DISTINCT artist) FROM tracks").fetchone()[0]
    albums = conn.execute("SELECT COUNT(DISTINCT album) FROM tracks").fetchone()[0]
    total_dur = conn.execute("SELECT COALESCE(SUM(duration),0) FROM tracks").fetchone()[0]
    total_bytes = sum(f.stat().st_size for f in MUSIC_DIR.rglob('*') if f.is_file() and f.suffix.lower() in ['.mp3','.flac','.ogg','.m4a','.wav','.aac','.wma'])
    return {"tracks": tracks, "artists": artists, "albums": albums, "total_duration": round(total_dur, 1), "disk_usage_bytes": total_bytes}

@app.post("/api/admin/upload")
async def admin_upload(files: list[UploadFile] = File(...), token: Optional[str] = Header(None)):
    _require_admin(token)
    imported = []
    for f in files:
        if not f.filename:
            continue
        ext = Path(f.filename).suffix.lower()
        if ext not in ['.mp3','.flac','.ogg','.m4a','.wav','.aac','.wma']:
            continue
        dest = MUSIC_DIR / f.filename
        content = await f.read()
        dest.write_bytes(content)
        try:
            process_file(str(dest))
            imported.append(f.filename)
        except Exception as e:
            imported.append(f"{f.filename}: error - {e}")
    return {"imported": imported}

@app.delete("/api/admin/tracks/{track_id}")
def admin_delete_track(track_id: int, token: Optional[str] = Header(None)):
    _require_admin(token)
    conn = get_connection()
    row = conn.execute("SELECT file_path FROM tracks WHERE id = ?", (track_id,)).fetchone()
    if not row:
        raise HTTPException(404, "Track not found")
    fp = row["file_path"]
    conn.execute("DELETE FROM playlist_tracks WHERE track_id = ?", (track_id,))
    conn.execute("DELETE FROM favorites WHERE track_id = ?", (track_id,))
    conn.execute("DELETE FROM tracks WHERE id = ?", (track_id,))
    conn.commit()
    if fp and Path(fp).exists():
        Path(fp).unlink()
    cover_p = COVERS_DIR / f"{track_id}.jpg"
    if cover_p.exists():
        cover_p.unlink()
    cover_p2 = COVERS_DIR / f"{track_id}.png"
    if cover_p2.exists():
        cover_p2.unlink()
    return {"deleted": track_id}

@app.get("/api/admin/dashboard")
def admin_dashboard(token: Optional[str] = Header(None)):
    _require_admin(token)
    conn = get_connection()

    # genre breakdown
    genres = conn.execute(
        """SELECT COALESCE(NULLIF(genre,''), 'Unknown') as genre, COUNT(*) as count
           FROM tracks GROUP BY genre ORDER BY count DESC"""
    ).fetchall()

    # tracks added per month (last 12)
    monthly = conn.execute(
        """SELECT strftime('%Y-%m', created_at) as month, COUNT(*) as count
           FROM tracks
           WHERE created_at >= date('now', '-12 months')
           GROUP BY month ORDER BY month"""
    ).fetchall()

    # plays per day (last 14)
    plays_day = conn.execute(
        """SELECT strftime('%Y-%m-%d', played_at) as day, COUNT(*) as count
           FROM play_history
           WHERE played_at >= date('now', '-14 days')
           GROUP BY day ORDER BY day"""
    ).fetchall()

    # disk usage by format
    disk_fmt = {}
    for f in MUSIC_DIR.rglob('*'):
        if f.is_file():
            ext = f.suffix.lower()
            if ext in ['.mp3','.flac','.ogg','.m4a','.wav','.aac','.wma']:
                disk_fmt[ext[1:].upper()] = disk_fmt.get(ext[1:].upper(), 0) + f.stat().st_size
    disk = [{"format": k, "bytes": v} for k, v in sorted(disk_fmt.items(), key=lambda x: -x[1])]

    # total users
    total_users = conn.execute("SELECT COUNT(*) FROM users").fetchone()[0]

    conn.close()

    disk_free = 0
    try:
        disk_free = shutil.disk_usage(str(MUSIC_DIR)).free
    except:
        pass

    return {
        "genres": [dict(r) for r in genres],
        "monthly_adds": [dict(r) for r in monthly],
        "plays_per_day": [dict(r) for r in plays_day],
        "disk_by_format": disk,
        "total_users": total_users,
        "disk_free": disk_free,
    }

@app.get("/api/admin/scanner")
def admin_scanner_status(token: Optional[str] = Header(None)):
    _require_admin(token)
    return get_scan_status()

@app.post("/api/admin/rescan")
def admin_rescan(token: Optional[str] = Header(None)):
    _require_admin(token)
    scan_existing()
    return get_scan_status()

@app.get("/api/admin/listening-trends")
def admin_listening_trends(token: Optional[str] = Header(None)):
    _require_admin(token)
    conn = get_connection()
    by_hour = conn.execute(
        """SELECT CAST(strftime('%H', played_at) AS INTEGER) as hour, COUNT(*) as count
           FROM play_history GROUP BY hour ORDER BY hour"""
    ).fetchall()
    by_day = conn.execute(
        """SELECT CASE CAST(strftime('%w', played_at) AS INTEGER)
                    WHEN 0 THEN 'Sun' WHEN 1 THEN 'Mon' WHEN 2 THEN 'Tue'
                    WHEN 3 THEN 'Wed' WHEN 4 THEN 'Thu' WHEN 5 THEN 'Fri'
                    ELSE 'Sat' END as day,
                COUNT(*) as count
           FROM play_history GROUP BY day ORDER BY MIN(played_at)"""
    ).fetchall()
    conn.close()
    return {
        "by_hour": [dict(r) for r in by_hour],
        "by_day": [dict(r) for r in by_day],
    }

@app.get("/api/admin/plays")
def admin_play_stats(token: Optional[str] = Header(None)):
    _require_admin(token)
    conn = get_connection()
    total = conn.execute("SELECT COALESCE(SUM(play_count),0) FROM tracks").fetchone()[0]

    top_tracks = conn.execute(
        """SELECT id, title, artist, album, play_count
           FROM tracks ORDER BY play_count DESC LIMIT 10"""
    ).fetchall()

    top_artists = conn.execute(
        """SELECT artist, COALESCE(SUM(play_count),0) as play_count
           FROM tracks WHERE artist IS NOT NULL
           GROUP BY artist ORDER BY play_count DESC LIMIT 10"""
    ).fetchall()

    top_albums = conn.execute(
        """SELECT album, artist, COALESCE(SUM(play_count),0) as play_count
           FROM tracks WHERE album IS NOT NULL
           GROUP BY album, artist ORDER BY play_count DESC LIMIT 10"""
    ).fetchall()

    # total listening time from play history (sum of durations of played tracks)
    listen = conn.execute(
        """SELECT COALESCE(SUM(t.duration),0) as total_time
           FROM play_history ph JOIN tracks t ON ph.track_id = t.id"""
    ).fetchone()[0]

    conn.close()
    return {
        "total_plays": total,
        "total_listen_time": round(listen, 1),
        "top_tracks": [dict(r) for r in top_tracks],
        "top_artists": [dict(r) for r in top_artists],
        "top_albums": [dict(r) for r in top_albums],
    }

@app.get("/api/admin/users")
def admin_list_users(token: Optional[str] = Header(None)):
    _require_admin(token)
    conn = get_connection()
    rows = conn.execute(
        """SELECT id, username, email, is_admin, created_at
           FROM users ORDER BY username"""
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]

class ToggleAdminBody(BaseModel):
    is_admin: bool

@app.post("/api/admin/users/{user_id}/admin")
def admin_toggle_user(user_id: int, body: ToggleAdminBody, token: Optional[str] = Header(None)):
    _require_admin(token)
    conn = get_connection()
    target = conn.execute("SELECT * FROM users WHERE id = ?", (user_id,)).fetchone()
    if not target:
        conn.close()
        raise HTTPException(404, "User not found")
    if target["username"] == "admin":
        conn.close()
        raise HTTPException(400, "Cannot change the root admin account")
    conn.execute("UPDATE users SET is_admin = ? WHERE id = ?", (1 if body.is_admin else 0, user_id))
    conn.commit()
    conn.close()
    return {"ok": True, "username": target["username"], "is_admin": body.is_admin}

class CreateUserBody(BaseModel):
    username: str
    password: str
    is_admin: bool = False

@app.post("/api/admin/users")
def admin_create_user(body: CreateUserBody, token: Optional[str] = Header(None)):
    _require_admin(token)
    conn = get_connection()
    existing = conn.execute("SELECT id FROM users WHERE username = ?", (body.username,)).fetchone()
    if existing:
        conn.close()
        raise HTTPException(400, "Username already taken")
    pwd_hash, salt = _hash_password(body.password)
    conn.execute(
        "INSERT INTO users (username, password_hash, salt, is_admin) VALUES (?, ?, ?, ?)",
        (body.username, pwd_hash, salt, 1 if body.is_admin else 0),
    )
    conn.commit()
    conn.close()
    return {"ok": True, "username": body.username, "is_admin": body.is_admin}

class ChangePasswordBody(BaseModel):
    password: str

@app.post("/api/admin/users/{user_id}/password")
def admin_change_password(user_id: int, body: ChangePasswordBody, token: Optional[str] = Header(None)):
    _require_admin(token)
    conn = get_connection()
    target = conn.execute("SELECT * FROM users WHERE id = ?", (user_id,)).fetchone()
    if not target:
        conn.close()
        raise HTTPException(404, "User not found")
    pwd_hash, salt = _hash_password(body.password)
    conn.execute("UPDATE users SET password_hash = ?, salt = ? WHERE id = ?", (pwd_hash, salt, user_id))
    conn.commit()
    conn.close()
    return {"ok": True, "username": target["username"]}

@app.delete("/api/admin/users/{user_id}")
def admin_delete_user(user_id: int, token: Optional[str] = Header(None)):
    _require_admin(token)
    conn = get_connection()
    target = conn.execute("SELECT * FROM users WHERE id = ?", (user_id,)).fetchone()
    if not target:
        conn.close()
        raise HTTPException(404, "User not found")
    if target["username"] == "admin":
        conn.close()
        raise HTTPException(400, "Cannot delete the root admin account")
    conn.execute("DELETE FROM sessions WHERE user_id = ?", (user_id,))
    conn.execute("DELETE FROM favorites WHERE user_id = ?", (user_id,))
    conn.execute("DELETE FROM playlist_tracks WHERE playlist_id IN (SELECT id FROM playlists WHERE user_id = ?)", (user_id,))
    conn.execute("DELETE FROM playlists WHERE user_id = ?", (user_id,))
    conn.execute("DELETE FROM users WHERE id = ?", (user_id,))
    conn.commit()
    conn.close()
    return {"ok": True, "username": target["username"]}

@app.get("/admin")
def admin_page():
    return FileResponse(str(STATIC_DIR / "admin.html"))

@app.get("/")
def index():
    return FileResponse(str(STATIC_DIR / "index.html"))


@app.get("/s/{token}")
def shared_playlist_page(token: str):
    return FileResponse(str(STATIC_DIR / "shared.html"))


@app.get("/api/player/now-playing")
def now_playing():
    conn = get_connection()
    row = conn.execute(
        """SELECT t.id, t.title, t.artist, t.album, t.duration
           FROM play_history ph
           JOIN tracks t ON ph.track_id = t.id
           ORDER BY ph.played_at DESC LIMIT 1"""
    ).fetchone()
    conn.close()
    if row:
        return dict(row)
    return {"id": None, "title": None, "artist": None, "album": None, "duration": None}


if __name__ == "__main__":
    uvicorn.run("server:app", host="0.0.0.0", port=PORT, reload=False)
