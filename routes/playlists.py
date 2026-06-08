import json
from typing import Optional

from fastapi import APIRouter, HTTPException, Header, UploadFile, File, Response, Query

from database import get_connection
from routes.deps import (
    _get_user_from_token, _safe_name,
    CreatePlaylistBody, AddTrackBody, ReorderPlaylistBody,
    CreateFolderBody, RenameFolderBody, SetPlaylistFolderBody,
    ShareAlbumBody, UnshareAlbumBody,
)

router = APIRouter(tags=["playlists"])


@router.get("/api/playlists")
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


@router.post("/api/playlists")
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


@router.delete("/api/playlists/{playlist_id}")
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
    conn.execute("DELETE FROM shared_playlists WHERE playlist_id = ?", (playlist_id,))
    conn.execute("DELETE FROM playlist_tracks WHERE playlist_id = ?", (playlist_id,))
    conn.execute("DELETE FROM playlists WHERE id = ?", (playlist_id,))
    conn.commit()
    conn.close()
    return {"ok": True}


@router.get("/api/playlists/{playlist_id}/tracks")
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


@router.post("/api/playlists/{playlist_id}/tracks")
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


@router.delete("/api/playlists/{playlist_id}/tracks/{track_id}")
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


@router.put("/api/playlists/{playlist_id}/tracks/reorder")
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
    for i, track_id in enumerate(body.order):
        conn.execute(
            "UPDATE playlist_tracks SET position = ? WHERE playlist_id = ? AND track_id = ?",
            (i + 1, playlist_id, track_id),
        )
    conn.commit()
    conn.close()
    return {"ok": True}


# Shared playlists

@router.post("/api/playlists/{playlist_id}/share")
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
    import secrets
    share_token = secrets.token_urlsafe(16)
    conn.execute(
        "INSERT INTO shared_playlists (token, playlist_id) VALUES (?, ?)",
        (share_token, playlist_id),
    )
    conn.commit()
    conn.close()
    return {"token": share_token}


@router.get("/api/playlists/{playlist_id}/share")
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


@router.delete("/api/playlists/{playlist_id}/share")
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


@router.get("/api/shared/{token}")
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
    return {"name": row["name"], "username": row["username"], "tracks": [dict(r) for r in tracks]}


# Shared albums

@router.post("/api/albums/share")
def share_album(body: ShareAlbumBody, token: Optional[str] = Header(None)):
    album = body.album.strip()
    artist = body.artist.strip() or None if body.artist else None
    if not album:
        raise HTTPException(400, "Album name required")
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Login required")
    conn = get_connection()
    existing = conn.execute(
        "SELECT token FROM shared_albums WHERE album = ? AND (artist IS ? OR artist = ?)",
        (album, artist, artist),
    ).fetchone()
    if existing:
        conn.close()
        return {"token": existing["token"]}
    import secrets
    share_token = secrets.token_urlsafe(16)
    conn.execute(
        "INSERT INTO shared_albums (token, album, artist, user_id) VALUES (?, ?, ?, ?)",
        (share_token, album, artist, user["id"] if user else None),
    )
    conn.commit()
    conn.close()
    return {"token": share_token}


@router.get("/api/album-shared/{token}")
def get_shared_album(token: str):
    conn = get_connection()
    row = conn.execute(
        "SELECT album, artist, user_id FROM shared_albums WHERE token = ?", (token,)
    ).fetchone()
    if not row:
        conn.close()
        raise HTTPException(404, "Shared album not found")
    artist = row["artist"]
    if artist:
        tracks = conn.execute(
            "SELECT * FROM tracks WHERE album = ? AND artist = ? ORDER BY disc_number, track_number",
            (row["album"], artist),
        ).fetchall()
    else:
        tracks = conn.execute(
            "SELECT * FROM tracks WHERE album = ? ORDER BY disc_number, track_number",
            (row["album"],),
        ).fetchall()
    conn.close()
    return {
        "album": row["album"],
        "artist": row["artist"],
        "tracks": [dict(r) for r in tracks],
    }


@router.delete("/api/albums/share")
def unshare_album(body: UnshareAlbumBody, token: Optional[str] = Header(None)):
    album = body.album.strip()
    artist = body.artist.strip() or None if body.artist else None
    if not album:
        raise HTTPException(400, "Album name required")
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Login required")
    conn = get_connection()
    conn.execute(
        "DELETE FROM shared_albums WHERE album = ? AND (artist IS ? OR artist = ?)",
        (album, artist, artist),
    )
    conn.commit()
    conn.close()
    return {"ok": True}


@router.get("/api/albums/share-status")
def get_album_share_status(album: str = Query(...), artist: Optional[str] = Query(None), token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    conn = get_connection()
    row = conn.execute(
        "SELECT token FROM shared_albums WHERE album = ? AND (artist IS ? OR artist = ?)",
        (album, artist, artist),
    ).fetchone()
    conn.close()
    return {"shared": row is not None, "token": row["token"] if row else None}


# Playlist folders

@router.get("/api/playlist-folders")
def list_folders(token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if user is None:
        return []
    conn = get_connection()
    rows = conn.execute(
        """SELECT pf.*, COUNT(p.id) as playlist_count
           FROM playlist_folders pf
           LEFT JOIN playlists p ON p.folder_id = pf.id
           WHERE pf.user_id = ?
           GROUP BY pf.id
           ORDER BY pf.sort_order, pf.name""",
        (user["id"],),
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


@router.post("/api/playlist-folders")
def create_folder(body: CreateFolderBody, token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Login required")
    conn = get_connection()
    max_order = conn.execute(
        "SELECT COALESCE(MAX(sort_order), -1) FROM playlist_folders WHERE user_id = ?",
        (user["id"],),
    ).fetchone()[0]
    conn.execute(
        "INSERT INTO playlist_folders (name, user_id, sort_order) VALUES (?, ?, ?)",
        (body.name, user["id"], max_order + 1),
    )
    conn.commit()
    fid = conn.execute("SELECT last_insert_rowid()").fetchone()[0]
    conn.close()
    return {"id": fid, "name": body.name}


@router.put("/api/playlist-folders/{folder_id}")
def rename_folder(folder_id: int, body: RenameFolderBody, token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Login required")
    conn = get_connection()
    row = conn.execute(
        "SELECT * FROM playlist_folders WHERE id = ? AND user_id = ?",
        (folder_id, user["id"]),
    ).fetchone()
    if not row:
        conn.close()
        raise HTTPException(404, "Folder not found")
    conn.execute("UPDATE playlist_folders SET name = ? WHERE id = ?", (body.name, folder_id))
    conn.commit()
    conn.close()
    return {"ok": True}


@router.delete("/api/playlist-folders/{folder_id}")
def delete_folder(folder_id: int, token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Login required")
    conn = get_connection()
    row = conn.execute(
        "SELECT * FROM playlist_folders WHERE id = ? AND user_id = ?",
        (folder_id, user["id"]),
    ).fetchone()
    if not row:
        conn.close()
        raise HTTPException(404, "Folder not found")
    conn.execute("UPDATE playlists SET folder_id = NULL WHERE folder_id = ?", (folder_id,))
    conn.execute("DELETE FROM playlist_folders WHERE id = ?", (folder_id,))
    conn.commit()
    conn.close()
    return {"ok": True}


@router.put("/api/playlists/{playlist_id}/folder")
def set_playlist_folder(playlist_id: int, body: SetPlaylistFolderBody, token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Login required")
    conn = get_connection()
    pl = conn.execute(
        "SELECT * FROM playlists WHERE id = ? AND user_id = ?",
        (playlist_id, user["id"]),
    ).fetchone()
    if not pl:
        conn.close()
        raise HTTPException(404, "Playlist not found")
    conn.execute("UPDATE playlists SET folder_id = ? WHERE id = ?", (body.folder_id, playlist_id))
    conn.commit()
    conn.close()
    return {"ok": True}


# Favorites

@router.get("/api/favorites")
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


@router.post("/api/favorites/{track_id}")
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


@router.delete("/api/favorites/{track_id}")
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


@router.get("/api/favorites/check/{track_id}")
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


# Play count

@router.post("/api/play/{track_id}")
def increment_play_count(track_id: int):
    conn = get_connection()
    conn.execute("UPDATE tracks SET play_count = COALESCE(play_count,0) + 1 WHERE id = ?", (track_id,))
    conn.execute("INSERT INTO play_history (track_id) VALUES (?)", (track_id,))
    conn.commit()
    conn.close()
    return {"ok": True}


# Playlist export / import

def _build_m3u(tracks_data, playlist_name):
    lines = ["#EXTM3U"]
    lines.append(f"#PLAYLIST: {playlist_name}")
    for t in tracks_data:
        dur = int(t["duration"]) if t.get("duration") else -1
        artist = t.get("artist") or "Unknown"
        title = t.get("title") or "Unknown"
        lines.append(f"#EXTINF:{dur},{artist} - {title}")
        lines.append(t.get("file_path") or "")
    return "\n".join(lines)


def _parse_m3u(content: str):
    entries = []
    lines = content.strip().split("\n")
    i = 0
    while i < len(lines):
        line = lines[i].strip()
        if line.startswith("#EXTINF:"):
            info = line[len("#EXTINF:"):]
            dur_and_title = info.split(",", 1)
            artist_title = dur_and_title[-1] if len(dur_and_title) > 1 else ""
            artist = ""
            title = artist_title
            if " - " in artist_title:
                parts = artist_title.split(" - ", 1)
                artist = parts[0].strip()
                title = parts[1].strip()
            i += 1
            while i < len(lines) and (lines[i].strip() == "" or lines[i].startswith("#")):
                i += 1
            path = lines[i].strip() if i < len(lines) else ""
            entries.append({"artist": artist, "title": title, "file_path": path})
        elif not line.startswith("#") and line:
            entries.append({"artist": "", "title": "", "file_path": line})
        i += 1
    return entries


@router.get("/api/playlists/{playlist_id}/export")
def export_playlist(playlist_id: int, format: str = Query("m3u"), token: Optional[str] = Header(None)):
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
    rows = conn.execute(
        """SELECT t.* FROM playlist_tracks pt
           JOIN tracks t ON pt.track_id = t.id
           WHERE pt.playlist_id = ?
           ORDER BY pt.position""",
        (playlist_id,),
    ).fetchall()
    conn.close()

    tracks_data = [dict(r) for r in rows]
    safe_name = _safe_name(pl["name"])

    if format == "json":
        payload = {
            "playlist_name": pl["name"],
            "format_version": 1,
            "tracks": [
                {
                    "title": t.get("title"),
                    "artist": t.get("artist"),
                    "album": t.get("album"),
                    "duration": t.get("duration"),
                    "file_path": t.get("file_path"),
                }
                for t in tracks_data
            ],
        }
        return Response(
            content=json.dumps(payload, indent=2),
            media_type="application/json",
            headers={"Content-Disposition": f'attachment; filename="{safe_name}.json"'},
        )

    content = _build_m3u(tracks_data, pl["name"])
    return Response(
        content=content,
        media_type="audio/x-mpegurl",
        headers={"Content-Disposition": f'attachment; filename="{safe_name}.m3u"'},
    )


@router.post("/api/playlists/import")
async def import_playlist(file: UploadFile = File(...), token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Login required")

    content = await file.read()
    filename = file.filename or "import"
    text = content.decode("utf-8", errors="replace")

    entries = []
    name_hint = filename.rsplit(".", 1)[0]

    if filename.endswith(".json"):
        try:
            data = json.loads(text)
            if isinstance(data, dict) and "tracks" in data:
                name_hint = data.get("playlist_name", name_hint)
                for t in data["tracks"]:
                    entries.append({
                        "artist": t.get("artist", ""),
                        "title": t.get("title", ""),
                        "file_path": t.get("file_path", ""),
                    })
            else:
                raise HTTPException(400, "Invalid JSON format")
        except json.JSONDecodeError:
            raise HTTPException(400, "Invalid JSON file")
    else:
        entries = _parse_m3u(text)

    if not entries:
        raise HTTPException(400, "No tracks found in file")

    conn = get_connection()
    conn.execute(
        "INSERT INTO playlists (name, user_id) VALUES (?, ?)",
        (name_hint, user["id"]),
    )
    conn.commit()
    pid = conn.execute("SELECT last_insert_rowid()").fetchone()[0]

    matched = 0
    position = 0
    for entry in entries:
        track_id = None
        if entry["file_path"]:
            row = conn.execute(
                "SELECT id FROM tracks WHERE file_path = ?", (entry["file_path"],)
            ).fetchone()
            if row:
                track_id = row["id"]

        if track_id is None and entry["title"]:
            if entry["artist"]:
                row = conn.execute(
                    "SELECT id FROM tracks WHERE title = ? AND artist = ?",
                    (entry["title"], entry["artist"]),
                ).fetchone()
            else:
                row = conn.execute(
                    "SELECT id FROM tracks WHERE title = ?", (entry["title"],)
                ).fetchone()
            if row:
                track_id = row["id"]

        if track_id is not None:
            position += 1
            conn.execute(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)",
                (pid, track_id, position),
            )
            matched += 1

    conn.commit()
    conn.close()
    return {"ok": True, "playlist_id": pid, "name": name_hint, "matched": matched, "total": len(entries)}
