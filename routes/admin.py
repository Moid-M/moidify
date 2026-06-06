import json
import os
import threading
import time
import uuid
from pathlib import Path
from typing import Optional

from fastapi import APIRouter, HTTPException, Header, UploadFile, File

from config import BASE_DIR, MUSIC_DIR, COVERS_DIR, MAX_UPLOAD_SIZE
from database import get_connection
from scanner import scan_existing, get_scan_status, process_file
from routes.deps import (
    _require_admin, _hash_password, _create_session, _validate_password,
    ScheduleBody, ToggleAdminBody, CreateUserBody, ChangePasswordBody,
)
from pydantic import BaseModel

DOWNLOAD_JOBS = {}
DOWNLOAD_LOCK = threading.Lock()
_MAX_DOWNLOAD_JOBS = 100


def _cleanup_old_jobs():
    with DOWNLOAD_LOCK:
        done_jobs = [jid for jid, j in DOWNLOAD_JOBS.items() if j.get("status") in ("done", "error")]
        for jid in done_jobs[:-50]:
            del DOWNLOAD_JOBS[jid]

class ImportUrlBody(BaseModel):
    url: str

def _download_worker(job_id: str, url: str):
    try:
        import yt_dlp
        import urllib.request
        from mutagen.id3 import ID3, APIC, TIT2, TPE1, TALB, TDRC, TCON, TRCK
        from mutagen.mp3 import MP3

        output_template = str(MUSIC_DIR / "%(title)s.%(ext)s")
        ydl_opts = {
            'format': 'bestaudio/best',
            'outtmpl': output_template,
            'quiet': True,
            'no_warnings': True,
            'postprocessors': [{
                'key': 'FFmpegExtractAudio',
                'preferredcodec': 'mp3',
                'preferredquality': '192',
            }],
            'progress_hooks': [_make_progress_hook(job_id)],
        }
        with DOWNLOAD_LOCK:
            DOWNLOAD_JOBS[job_id]["status"] = "downloading"
        with yt_dlp.YoutubeDL(ydl_opts) as ydl:
            info = ydl.extract_info(url, download=True)
            title = info.get("title", "Unknown")
            artist = info.get("uploader") or info.get("channel") or info.get("artist") or "Unknown"
            album = info.get("album") or ""
            year = str(info.get("release_year") or info.get("year") or "")
            genre = info.get("genre") or ""
            track_num = str(info.get("track_number") or info.get("playlist_index") or "")
            thumbnail_url = info.get("thumbnail") or ""

        # Find the output file
        outfile = None
        for f in MUSIC_DIR.iterdir():
            if f.suffix.lower() == ".mp3" and f.name.startswith(info.get("title", "")[:40]):
                outfile = f
                break
        if not outfile:
            for f in MUSIC_DIR.iterdir():
                if f.suffix.lower() == ".mp3":
                    outfile = f
                    break

        if outfile:
            try:
                tags = ID3(outfile)
            except Exception:
                tags = ID3()
            tags["TIT2"] = TIT2(encoding=3, text=title)
            tags["TPE1"] = TPE1(encoding=3, text=artist)
            if album:
                tags["TALB"] = TALB(encoding=3, text=album)
            if year:
                tags["TDRC"] = TDRC(encoding=3, text=year)
            if genre:
                tags["TCON"] = TCON(encoding=3, text=genre)
            if track_num:
                tags["TRCK"] = TRCK(encoding=3, text=track_num)
            # Download and embed cover art
            if thumbnail_url:
                try:
                    req = urllib.request.Request(thumbnail_url, headers={"User-Agent": "Mozilla/5.0"})
                    img_data = urllib.request.urlopen(req, timeout=15).read()
                    mime = "image/jpeg"
                    if thumbnail_url.endswith(".png"):
                        mime = "image/png"
                    if thumbnail_url.endswith(".webp"):
                        mime = "image/webp"
                    tags["APIC"] = APIC(encoding=3, mime=mime, type=3, desc="Cover", data=img_data)
                except Exception:
                    pass
            tags.save(outfile)

        with DOWNLOAD_LOCK:
            DOWNLOAD_JOBS[job_id]["title"] = f"{title} — {artist}"
            DOWNLOAD_JOBS[job_id]["status"] = "importing"
        # Trigger a scan to pick up the new file
        scan_existing()
        with DOWNLOAD_LOCK:
            DOWNLOAD_JOBS[job_id]["status"] = "done"
    except Exception as e:
        with DOWNLOAD_LOCK:
            DOWNLOAD_JOBS[job_id]["status"] = "error"
            DOWNLOAD_JOBS[job_id]["error"] = str(e)

def _make_progress_hook(job_id: str):
    def hook(d):
        if d.get('status') == 'downloading':
            total = d.get('total_bytes') or d.get('total_bytes_estimate') or 0
            downloaded = d.get('downloaded_bytes', 0)
            pct = (downloaded / total * 100) if total else 0
            with DOWNLOAD_LOCK:
                DOWNLOAD_JOBS[job_id]["progress"] = round(pct, 1)
    return hook

router = APIRouter(tags=["admin"])

AUDIO_EXTS = {'.mp3', '.flac', '.ogg', '.m4a', '.wav', '.aac', '.wma'}

_DISK_CACHE = {"bytes": 0, "by_format": {}, "timestamp": 0}
_DISK_CACHE_TTL = 300


def _walk_music_dir():
    now = __import__("time").time()
    if now - _DISK_CACHE["timestamp"] < _DISK_CACHE_TTL:
        return _DISK_CACHE["bytes"], _DISK_CACHE["by_format"]
    total = 0
    fmt = {}
    try:
        for root, dirs, files in os.walk(str(MUSIC_DIR)):
            for fname in files:
                ext = Path(fname).suffix.lower()
                if ext in AUDIO_EXTS:
                    try:
                        sz = os.path.getsize(os.path.join(root, fname))
                        total += sz
                        fmt[ext[1:].upper()] = fmt.get(ext[1:].upper(), 0) + sz
                    except Exception:
                        pass
    except Exception:
        pass
    _DISK_CACHE["bytes"] = total
    _DISK_CACHE["by_format"] = fmt
    _DISK_CACHE["timestamp"] = now
    return total, fmt


@router.get("/api/admin/stats")
def admin_stats(token: Optional[str] = Header(None)):
    _require_admin(token)
    conn = get_connection()
    tracks = conn.execute("SELECT COUNT(*) FROM tracks").fetchone()[0]
    artists = conn.execute("SELECT COUNT(DISTINCT COALESCE(NULLIF(album_artist,''), artist)) FROM tracks").fetchone()[0]
    albums = conn.execute("SELECT COUNT(DISTINCT album || COALESCE(NULLIF(album_artist,''), artist)) FROM tracks").fetchone()[0]
    total_dur = conn.execute("SELECT COALESCE(SUM(duration),0) FROM tracks").fetchone()[0]
    conn.close()

    total_bytes, _ = _walk_music_dir()
    return {"tracks": tracks, "artists": artists, "albums": albums,
            "total_duration": round(total_dur, 1), "disk_usage_bytes": total_bytes}


@router.post("/api/admin/upload")
async def admin_upload(files: list[UploadFile] = File(...), token: Optional[str] = Header(None)):
    _require_admin(token)
    imported = []
    total = 0
    limit_gb = MAX_UPLOAD_SIZE / (1024 * 1024 * 1024)
    for f in files:
        if not f.filename:
            continue
        ext = Path(f.filename).suffix.lower()
        if ext not in ['.mp3','.flac','.ogg','.m4a','.wav','.aac','.wma']:
            continue
        dest = MUSIC_DIR / Path(f.filename).name
        content = await f.read()
        total += len(content)
        if total > MAX_UPLOAD_SIZE:
            raise HTTPException(413, f"Upload too large (max {limit_gb:.1f} GB total)")
        try:
            if dest.exists():
                dest.unlink()
            dest.write_bytes(content)
            conn = get_connection()
            conn.execute("DELETE FROM tracks WHERE file_path = ?", (str(dest),))
            conn.commit()
            conn.close()
            process_file(str(dest))
            imported.append(f.filename)
        except Exception as e:
            if dest.exists():
                dest.unlink()
            imported.append(f"{f.filename}: error - {e}")
    return {"imported": imported}


@router.delete("/api/admin/tracks/{track_id}")
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
    conn.close()
    if fp and Path(fp).exists():
        Path(fp).unlink()
    for ext in (".jpg", ".png", ".webp"):
        p = COVERS_DIR / f"{track_id}{ext}"
        if p.exists():
            p.unlink()
    return {"deleted": track_id}


@router.get("/api/admin/dashboard")
def admin_dashboard(token: Optional[str] = Header(None)):
    _require_admin(token)
    conn = get_connection()

    genres = conn.execute(
        """SELECT COALESCE(NULLIF(genre,''), 'Unknown') as genre, COUNT(*) as count
           FROM tracks GROUP BY genre ORDER BY count DESC"""
    ).fetchall()

    monthly = conn.execute(
        """SELECT strftime('%Y-%m', created_at) as month, COUNT(*) as count
           FROM tracks
           WHERE created_at >= date('now', '-12 months')
           GROUP BY month ORDER BY month"""
    ).fetchall()

    plays_day = conn.execute(
        """SELECT strftime('%Y-%m-%d', played_at) as day, COUNT(*) as count
           FROM play_history
           WHERE played_at >= date('now', '-14 days')
           GROUP BY day ORDER BY day"""
    ).fetchall()

    total_users = conn.execute("SELECT COUNT(*) FROM users").fetchone()[0]

    conn.close()

    _, disk_fmt = _walk_music_dir()
    disk = [{"format": k, "bytes": v} for k, v in sorted(disk_fmt.items(), key=lambda x: -x[1])]

    disk_free = 0
    try:
        import shutil
        disk_free = shutil.disk_usage(str(MUSIC_DIR)).free
    except Exception:
        pass

    return {
        "genres": [dict(r) for r in genres],
        "monthly_adds": [dict(r) for r in monthly],
        "plays_per_day": [dict(r) for r in plays_day],
        "disk_by_format": disk,
        "total_users": total_users,
        "disk_free": disk_free,
    }


@router.get("/api/admin/scanner")
def admin_scanner_status(token: Optional[str] = Header(None)):
    _require_admin(token)
    return get_scan_status()


@router.post("/api/admin/rescan")
def admin_rescan(token: Optional[str] = Header(None)):
    _require_admin(token)
    scan_existing()
    return get_scan_status()


# Rescan scheduler

SCHEDULE_FILE = BASE_DIR / "data" / "rescan_schedule.json"
_SCAN_THREAD = None
_SCAN_STOP = threading.Event()


def _load_schedule():
    if SCHEDULE_FILE.exists():
        try:
            with open(SCHEDULE_FILE) as f:
                return json.load(f)
        except Exception:
            pass
    return {"interval_hours": 0, "last_scan": None}


def _save_schedule(data):
    SCHEDULE_FILE.parent.mkdir(parents=True, exist_ok=True)
    with open(SCHEDULE_FILE, "w") as f:
        json.dump(data, f)


def _rescan_loop():
    while not _SCAN_STOP.is_set():
        schedule = _load_schedule()
        interval = schedule.get("interval_hours", 0)
        if interval > 0:
            scan_existing()
            schedule["last_scan"] = time.strftime('%Y-%m-%d %H:%M:%S')
            _save_schedule(schedule)
            _SCAN_STOP.wait(interval * 3600)
        else:
            _SCAN_STOP.wait(60)


def start_rescan_scheduler():
    global _SCAN_THREAD
    if _SCAN_THREAD and _SCAN_THREAD.is_alive():
        return
    _SCAN_STOP.clear()
    _SCAN_THREAD = threading.Thread(target=_rescan_loop, daemon=True)
    _SCAN_THREAD.start()


@router.get("/api/admin/schedule-rescan")
def get_rescan_schedule(token: Optional[str] = Header(None)):
    _require_admin(token)
    return _load_schedule()


@router.post("/api/admin/schedule-rescan")
def set_rescan_schedule(body: ScheduleBody, token: Optional[str] = Header(None)):
    _require_admin(token)
    if body.interval_hours < 0:
        raise HTTPException(400, "Interval must be >= 0")
    schedule = _load_schedule()
    schedule["interval_hours"] = body.interval_hours
    _save_schedule(schedule)
    start_rescan_scheduler()
    return {"ok": True, "interval_hours": body.interval_hours}


@router.get("/api/admin/listening-trends")
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
    return {"by_hour": [dict(r) for r in by_hour], "by_day": [dict(r) for r in by_day]}


@router.get("/api/admin/plays")
def admin_play_stats(token: Optional[str] = Header(None)):
    _require_admin(token)
    conn = get_connection()
    total = conn.execute("SELECT COALESCE(SUM(play_count),0) FROM tracks").fetchone()[0]

    top_tracks = conn.execute(
        """SELECT id, title, artist, album, play_count
           FROM tracks ORDER BY play_count DESC LIMIT 10"""
    ).fetchall()

    top_artists = conn.execute(
        """SELECT COALESCE(NULLIF(album_artist,''), artist) as artist, COALESCE(SUM(play_count),0) as play_count
           FROM tracks WHERE artist IS NOT NULL
           GROUP BY artist ORDER BY play_count DESC LIMIT 10"""
    ).fetchall()

    top_albums = conn.execute(
        """SELECT album, COALESCE(NULLIF(album_artist,''), artist) as artist, COALESCE(SUM(play_count),0) as play_count
           FROM tracks WHERE album IS NOT NULL
           GROUP BY album, artist ORDER BY play_count DESC LIMIT 10"""
    ).fetchall()

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


@router.get("/api/admin/users")
def admin_list_users(token: Optional[str] = Header(None)):
    _require_admin(token)
    conn = get_connection()
    rows = conn.execute(
        """SELECT id, username, email, is_admin, created_at
           FROM users ORDER BY username"""
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


@router.post("/api/admin/users/{user_id}/admin")
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


@router.post("/api/admin/users")
def admin_create_user(body: CreateUserBody, token: Optional[str] = Header(None)):
    _require_admin(token)
    _validate_password(body.password)
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


@router.post("/api/admin/users/{user_id}/password")
def admin_change_password(user_id: int, body: ChangePasswordBody, token: Optional[str] = Header(None)):
    _require_admin(token)
    _validate_password(body.password)
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


@router.delete("/api/admin/users/{user_id}")
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
    conn.execute("DELETE FROM shared_albums WHERE user_id = ?", (user_id,))
    conn.execute("DELETE FROM shared_playlists WHERE playlist_id IN (SELECT id FROM playlists WHERE user_id = ?)", (user_id,))
    conn.execute("DELETE FROM playlist_tracks WHERE playlist_id IN (SELECT id FROM playlists WHERE user_id = ?)", (user_id,))
    conn.execute("DELETE FROM playlists WHERE user_id = ?", (user_id,))
    conn.execute("DELETE FROM users WHERE id = ?", (user_id,))
    conn.commit()
    conn.close()
    return {"ok": True, "username": target["username"]}


@router.post("/api/admin/import-url")
def import_from_url(body: ImportUrlBody, token: Optional[str] = Header(None)):
    _require_admin(token)
    _cleanup_old_jobs()
    job_id = uuid.uuid4().hex[:12]
    with DOWNLOAD_LOCK:
        if len(DOWNLOAD_JOBS) >= _MAX_DOWNLOAD_JOBS:
            raise HTTPException(503, "Too many download jobs. Wait for some to complete.")
        DOWNLOAD_JOBS[job_id] = {"status": "queued", "progress": 0, "title": None, "error": None}
    threading.Thread(target=_download_worker, args=(job_id, body.url), daemon=True).start()
    return {"job_id": job_id, "status": "queued"}


@router.get("/api/admin/import-status/{job_id}")
def get_import_status(job_id: str, token: Optional[str] = Header(None)):
    _require_admin(token)
    with DOWNLOAD_LOCK:
        job = DOWNLOAD_JOBS.get(job_id)
    if not job:
        raise HTTPException(404, "Job not found")
    return job


@router.get("/api/admin/registration-status")
def admin_registration_status(token: Optional[str] = Header(None)):
    _require_admin(token)
    from routes.deps import _is_registration_open
    return {"registration_open": _is_registration_open()}


@router.post("/api/admin/registration-toggle")
def admin_toggle_registration(token: Optional[str] = Header(None)):
    _require_admin(token)
    conn = get_connection()
    current = conn.execute(
        "SELECT value FROM settings WHERE key = 'registration_open'"
    ).fetchone()
    new_val = "0" if (current and current["value"] == "0") else "1"
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES ('registration_open', ?)",
        (new_val,),
    )
    conn.commit()
    conn.close()
    return {"registration_open": new_val == "1"}


@router.get("/api/admin/failed-logins")
def admin_failed_logins(token: Optional[str] = Header(None)):
    _require_admin(token)
    conn = get_connection()
    rows = conn.execute(
        """SELECT id, username, failed_attempts, locked_until
           FROM users WHERE failed_attempts > 0 OR locked_until IS NOT NULL
           ORDER BY failed_attempts DESC"""
    ).fetchall()
    conn.close()
    return [{
        "id": r["id"],
        "username": r["username"],
        "failed_attempts": r["failed_attempts"],
        "locked_until": r["locked_until"],
        "locked": r["locked_until"] is not None and r["locked_until"] > int(__import__("time").time()),
    } for r in rows]


@router.post("/api/admin/users/{user_id}/unlock")
def admin_unlock_user(user_id: int, token: Optional[str] = Header(None)):
    _require_admin(token)
    conn = get_connection()
    conn.execute(
        "UPDATE users SET failed_attempts = 0, locked_until = NULL WHERE id = ?",
        (user_id,),
    )
    conn.commit()
    conn.close()
    return {"ok": True}


@router.get("/api/admin/users/{user_id}/sessions")
def admin_user_sessions(user_id: int, token: Optional[str] = Header(None)):
    _require_admin(token)
    conn = get_connection()
    rows = conn.execute(
        """SELECT token, user_id, created_at, expires_at
           FROM sessions WHERE user_id = ?
           ORDER BY created_at DESC""",
        (user_id,),
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


@router.delete("/api/admin/users/{user_id}/sessions")
def admin_revoke_user_sessions(user_id: int, token: Optional[str] = Header(None)):
    _require_admin(token)
    conn = get_connection()
    conn.execute("DELETE FROM sessions WHERE user_id = ?", (user_id,))
    conn.commit()
    conn.close()
    return {"ok": True, "message": "All sessions revoked"}
