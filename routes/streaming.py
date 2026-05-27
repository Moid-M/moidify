import os
import subprocess
import tempfile
import zipfile
from pathlib import Path
from typing import Optional

from fastapi import APIRouter, HTTPException, Query
from fastapi.responses import FileResponse, StreamingResponse

from config import MUSIC_DIR, COVERS_DIR, STATIC_DIR
from database import get_connection
from routes.deps import TRANSCODE_MAP, FFMPEG_PATH

router = APIRouter(tags=["streaming"])


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
            proc.wait(timeout=5)


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

    music_dir = Path(MUSIC_DIR).resolve()
    if not str(path).startswith(str(music_dir)):
        raise HTTPException(403, "Forbidden")

    if quality != "original" and quality in TRANSCODE_MAP and FFMPEG_PATH:
        return StreamingResponse(
            _stream_file(path, quality),
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
    safe_name = "".join(c if c.isalnum() or c in " _-" else "_" for c in album + suffix).strip()
    zip_name = safe_name + ".zip"

    tmp = tempfile.NamedTemporaryFile(delete=False, suffix=".zip")
    try:
        with zipfile.ZipFile(tmp, "w", zipfile.ZIP_DEFLATED) as zf:
            for row in rows:
                path = Path(row["file_path"])
                if path.exists():
                    prefix = f"{row['disc_number'] or 1}-{row['track_number'] or 0:02d}" if row['disc_number'] and row['disc_number'] > 1 else f"{row['track_number'] or 0:02d}"
                    track_name = f"{prefix} - {row['title'] or path.stem}{path.suffix}"
                    zf.write(str(path), track_name)
        tmp.close()
        return FileResponse(
            tmp.name,
            media_type="application/zip",
            filename=zip_name,
            headers={"Content-Disposition": f'attachment; filename="{zip_name}"'},
        )
    finally:
        try:
            os.unlink(tmp.name)
        except Exception:
            pass
