import subprocess
import threading
import zipfile
from pathlib import Path
from typing import Optional

from fastapi import APIRouter, HTTPException, Query
from fastapi.responses import FileResponse, StreamingResponse

from config import MUSIC_DIR, COVERS_DIR, STATIC_DIR
from database import get_connection
from routes.deps import TRANSCODE_MAP, FFMPEG_PATH

router = APIRouter(tags=["streaming"])

MUSIC_DIR_RESOLVED = Path(MUSIC_DIR).resolve(strict=False)

# Limit concurrent transcodes to prevent resource exhaustion
_active_transcodes = 0
_MAX_TRANSCODES = 10
_transcode_lock = threading.Lock()


def _within_music_dir(path: Path) -> bool:
    try:
        path.resolve().relative_to(MUSIC_DIR_RESOLVED)
        return True
    except ValueError:
        return False


def _transcode_stream(file_path: str, quality: str):
    if not FFMPEG_PATH or quality == "original" or quality not in TRANSCODE_MAP:
        return None
    cfg = TRANSCODE_MAP[quality]
    fmt = "ogg" if cfg["codec"] == "libopus" else "mp3"
    args = [
        FFMPEG_PATH, "-i", file_path,
        "-f", fmt,
        "-acodec", cfg["codec"],
        "-ab", cfg["bitrate"],
        "-vn", "-nostdin", "-loglevel", "error", "-",
    ]
    process = subprocess.Popen(
        args, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL
    )
    return process


def _stream_file(path: Path, quality: str):
    proc = None
    try:
        proc = _transcode_stream(str(path), quality)
        if proc:
            for chunk in iter(lambda: proc.stdout.read(65536), b""):
                yield chunk
    finally:
        if proc:
            proc.kill()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()


def _media_type_for(path: Path, quality: str) -> str:
    if quality != "original" and quality in TRANSCODE_MAP and TRANSCODE_MAP[quality]["codec"] == "libopus":
        return "audio/ogg"
    ext = path.suffix.lower()
    return {
        ".mp3": "audio/mpeg",
        ".flac": "audio/flac",
        ".ogg": "audio/ogg",
        ".m4a": "audio/mp4",
        ".wav": "audio/wav",
        ".wma": "audio/x-ms-wma",
        ".aac": "audio/aac",
    }.get(ext, "application/octet-stream")


@router.get("/api/stream/{track_id}")
def stream_track(track_id: int, quality: Optional[str] = Query("high")):
    conn = get_connection()
    row = conn.execute("SELECT file_path FROM tracks WHERE id = ?", (track_id,)).fetchone()
    conn.close()
    if row is None:
        raise HTTPException(404, "Track not found")

    path = Path(row["file_path"]).resolve()
    if not path.exists():
        raise HTTPException(404, "File not found on disk")

    if not _within_music_dir(path):
        raise HTTPException(403, "Forbidden")

    if quality != "original" and quality in TRANSCODE_MAP and FFMPEG_PATH:
        with _transcode_lock:
            if _active_transcodes >= _MAX_TRANSCODES:
                raise HTTPException(503, "Too many concurrent transcodes. Try again later.")
            _active_transcodes += 1

        def stream_with_cleanup():
            try:
                yield from _stream_file(path, quality)
            finally:
                with _transcode_lock:
                    global _active_transcodes
                    _active_transcodes -= 1

        return StreamingResponse(
            stream_with_cleanup(),
            media_type=_media_type_for(path, quality),
            headers={
                "Content-Disposition": "inline",
                "Cache-Control": "no-cache",
                "X-Transcoded": quality,
            },
        )

    return FileResponse(str(path), media_type=_media_type_for(path, "original"))


@router.get("/api/cover/{track_id}")
def get_cover(track_id: int):
    from database import get_connection
    conn = get_connection()
    row = conn.execute("SELECT cover_hash FROM tracks WHERE id = ?", (track_id,)).fetchone()
    conn.close()
    if row and row["cover_hash"]:
        for ext in (".jpg", ".png", ".webp"):
            p = COVERS_DIR / f"{row['cover_hash']}{ext}"
            if p.exists():
                mt = {"jpg": "image/jpeg", "png": "image/png", "webp": "image/webp"}.get(ext[1:], "image/jpeg")
                return FileResponse(str(p), media_type=mt, headers={"Cache-Control": "public, max-age=86400"})
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


@router.get("/api/download/album")
def download_album(album: str = Query(...), artist: Optional[str] = Query(None)):
    import io

    conn = get_connection()
    try:
        if artist:
            rows = conn.execute(
                "SELECT * FROM tracks WHERE album = ? AND artist = ? ORDER BY track_number",
                (album, artist),
            ).fetchall()
        else:
            rows = conn.execute(
                "SELECT * FROM tracks WHERE album = ? ORDER BY track_number", (album,)
            ).fetchall()
    finally:
        conn.close()

    if not rows:
        raise HTTPException(404, "No tracks found for this album")

    suffix = f"_{artist}" if artist else ""
    safe_name = "".join(c if c.isalnum() or c in " _-" else "_" for c in album + suffix).strip()
    zip_name = safe_name + ".zip"

    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as zf:
        for row in rows:
            path = Path(row["file_path"])
            if path.exists():
                prefix = f"{row['disc_number'] or 1}-{row['track_number'] or 0:02d}" if row['disc_number'] and row['disc_number'] > 1 else f"{row['track_number'] or 0:02d}"
                track_name = f"{prefix} - {row['title'] or path.stem}{path.suffix}"
                safe_name = "".join(c if c.isalnum() or c in " ._-" else "_" for c in track_name)
                zf.write(str(path), safe_name)
    buf.seek(0)
    return StreamingResponse(
        buf,
        media_type="application/zip",
        headers={"Content-Disposition": f'attachment; filename="{safe_name}.zip"'},
    )


@router.get("/api/download/tracks-zip")
def download_tracks_zip(ids: str = Query(...)):
    import io

    track_ids = [int(x) for x in ids.split(",") if x.strip().isdigit()]
    if not track_ids:
        raise HTTPException(400, "No valid track IDs")
    placeholders = ",".join("?" * len(track_ids))
    conn = get_connection()
    rows = conn.execute(f"SELECT * FROM tracks WHERE id IN ({placeholders})", track_ids).fetchall()
    conn.close()

    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as zf:
        for row in rows:
            path = Path(row["file_path"])
            if path.exists():
                prefix = f"{row['track_number'] or 0:02d}"
                track_name = f"{prefix} - {row['title'] or path.stem}{path.suffix}"
                safe_name = "".join(c if c.isalnum() or c in " ._-" else "_" for c in track_name)
                zf.write(str(path), safe_name)
    buf.seek(0)
    return StreamingResponse(
        buf,
        media_type="application/zip",
        headers={"Content-Disposition": 'attachment; filename="tracks.zip"'},
    )


@router.get("/api/download/track-zip/{track_id}")
def download_track_zip(track_id: int):
    import io

    conn = get_connection()
    row = conn.execute("SELECT * FROM tracks WHERE id = ?", (track_id,)).fetchone()
    conn.close()
    if not row:
        raise HTTPException(404, "Track not found")
    path = Path(row["file_path"])
    if not path.exists():
        raise HTTPException(404, "File not found on disk")

    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as zf:
        arcname = f"{row['title'] or path.stem}{path.suffix}"
        safe_arcname = "".join(c if c.isalnum() or c in " ._-" else "_" for c in arcname)
        zf.write(str(path), safe_arcname)
    buf.seek(0)
    safe_name = "".join(c if c.isalnum() or c in " _-" else "_" for c in (row["title"] or "track")).strip()
    return StreamingResponse(
        buf,
        media_type="application/zip",
        headers={"Content-Disposition": f'attachment; filename="{safe_name}.zip"'},
    )


@router.get("/api/download/playlist/{playlist_id}")
def download_playlist(playlist_id: int):
    import io

    conn = get_connection()
    rows = conn.execute(
        "SELECT t.* FROM tracks t JOIN playlist_tracks pt ON t.id = pt.track_id WHERE pt.playlist_id = ? ORDER BY pt.position",
        (playlist_id,),
    ).fetchall()
    pl_name = conn.execute("SELECT name FROM playlists WHERE id = ?", (playlist_id,)).fetchone()
    conn.close()
    if not rows:
        raise HTTPException(404, "No tracks in playlist")

    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as zf:
        for row in rows:
            path = Path(row["file_path"])
            if path.exists():
                prefix = f"{row['disc_number'] or 1}-{row['track_number'] or 0:02d}" if row['disc_number'] and row['disc_number'] > 1 else f"{row['track_number'] or 0:02d}"
                track_name = f"{prefix} - {row['title'] or path.stem}{path.suffix}"
                safe_name = "".join(c if c.isalnum() or c in " ._-" else "_" for c in track_name)
                zf.write(str(path), safe_name)
    buf.seek(0)
    name = pl_name["name"] if pl_name else "playlist"
    safe_name = "".join(c if c.isalnum() or c in " _-" else "_" for c in name).strip()
    return StreamingResponse(
        buf,
        media_type="application/zip",
        headers={"Content-Disposition": f'attachment; filename="{safe_name}.zip"'},
    )
