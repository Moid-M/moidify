from pathlib import Path
from typing import Optional
import re

import mutagen
from fastapi import APIRouter, HTTPException, Query, Header, Response
from fastapi.responses import FileResponse

from database import get_connection
from config import COVERS_DIR
from routes.deps import _normalize, _get_user_from_token, _fetch_lyrics_from_lrclib, RatingBody

router = APIRouter(tags=["tracks"])

_EDITION_PATTERN = re.compile(
    r'\s*[\(\[]\s*\d*\s*('
    r'Deluxe\s*(Edition|Version)?'
    r'|Remaster(ed)?'
    r'|Expanded\s*(Edition|Version)?'
    r'|Bonus\s*Track\s*(Edition|Version)?'
    r'|Special\s*(Edition|Version)?'
    r'|Collector\'?s?\s*(Edition|Version)?'
    r'|Limited\s*(Edition|Version)?'
    r'|Super\s*Deluxe'
    r'|Disk\s*\d+|Disc\s*\d+'
    r'|Version\s*\d+'
    r'|Anniversary\s*(Edition)?'
    r'|Reissue'
    r'|Original|Single|Explicit|Clean'
    r')\s*[\)\]]',
    re.IGNORECASE
)

def _base_album_name(name):
    if not name:
        return name
    return _EDITION_PATTERN.sub('', name).strip()


def _escape_like(s: str) -> str:
    return s.replace("\\", "\\\\").replace("%", "\\%").replace("_", "\\_")


@router.get("/api/tracks")
def list_tracks(
    search: Optional[str] = None,
    limit: int = Query(0, ge=0),
    offset: int = Query(0, ge=0),
):
    conn = get_connection()
    limit_clause = ""
    if limit > 0:
        limit_clause = " LIMIT ? OFFSET ?"
    if search:
        safe = _escape_like(search)
        normalized = _normalize(search)
        like_pattern = f"%{safe}%"
        norm_pattern = f"%{_escape_like(normalized)}%"
        base_sql = "SELECT * FROM tracks WHERE"
        if normalized.lower() != search.lower():
            rows = conn.execute(
                f"""{base_sql} title LIKE ? ESCAPE '\\' OR artist LIKE ? ESCAPE '\\' OR album LIKE ? ESCAPE '\\'
                   OR title LIKE ? ESCAPE '\\' OR artist LIKE ? ESCAPE '\\' OR album LIKE ? ESCAPE '\\'
                   ORDER BY COALESCE(NULLIF(album_artist,''), artist), album, disc_number, track_number{limit_clause}""",
                (like_pattern, like_pattern, like_pattern,
                 norm_pattern, norm_pattern, norm_pattern) + ((limit, offset) if limit > 0 else ()),
            ).fetchall()
        else:
            rows = conn.execute(
                f"""{base_sql} title LIKE ? ESCAPE '\\' OR artist LIKE ? ESCAPE '\\' OR album LIKE ? ESCAPE '\\'
                   ORDER BY COALESCE(NULLIF(album_artist,''), artist), album, disc_number, track_number{limit_clause}""",
                (like_pattern, like_pattern, like_pattern) + ((limit, offset) if limit > 0 else ()),
            ).fetchall()
    else:
        rows = conn.execute(
            f"""SELECT * FROM tracks
                ORDER BY COALESCE(NULLIF(album_artist,''), artist), album, disc_number, track_number{limit_clause}""",
            (limit, offset) if limit > 0 else ()
        ).fetchall()
    total = conn.execute("SELECT COUNT(*) as c FROM tracks").fetchone()["c"]
    conn.close()
    result = [dict(r) for r in rows]
    if limit > 0:
        return {"tracks": result, "total": total, "limit": limit, "offset": offset}
    return result


@router.get("/api/tracks/{track_id}")
def get_track(track_id: int):
    conn = get_connection()
    row = conn.execute("SELECT * FROM tracks WHERE id = ?", (track_id,)).fetchone()
    conn.close()
    if row is None:
        raise HTTPException(404, "Track not found")
    return dict(row)


@router.get("/api/tracks/{track_id}/similar")
def get_similar_tracks(track_id: int, limit: int = 20):
    conn = get_connection()
    track = conn.execute("SELECT * FROM tracks WHERE id = ?", (track_id,)).fetchone()
    if track is None:
        conn.close()
        raise HTTPException(404, "Track not found")
    match_artist = track["album_artist"] if track["album_artist"] else track["artist"]
    rows = conn.execute(
        """SELECT DISTINCT * FROM tracks
           WHERE id != ? AND (genre = ? OR artist = ?)
           ORDER BY CASE WHEN artist = ? THEN 0 ELSE 1 END, RANDOM()
           LIMIT ?""",
        (track_id, track["genre"], match_artist, match_artist, limit),
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


@router.get("/api/tracks/{track_id}/gain")
def get_track_gain(track_id: int):
    conn = get_connection()
    row = conn.execute("SELECT file_path FROM tracks WHERE id = ?", (track_id,)).fetchone()
    conn.close()
    if row is None:
        raise HTTPException(404, "Track not found")
    fp = row["file_path"]
    gain = None
    try:
        audio = mutagen.File(fp)
        if audio is not None:
            tags = audio.tags
            if tags:
                for key in tags:
                    if 'REPLAYGAIN_TRACK_GAIN' in key.upper():
                        val = str(tags[key])
                        if isinstance(val, str) and val.strip():
                            gain = float(val.strip().replace(' dB', ''))
                        break
    except Exception:
        pass
    return {"gain": gain}


@router.get("/api/tracks/{track_id}/lyrics")
def get_track_lyrics(track_id: int):
    conn = get_connection()
    row = conn.execute("SELECT lyrics, artist, title, album FROM tracks WHERE id = ?", (track_id,)).fetchone()
    if row is None:
        conn.close()
        raise HTTPException(404, "Track not found")
    lyrics = row["lyrics"]
    if not lyrics:
        lyrics = _fetch_lyrics_from_lrclib(row["artist"] or "", row["title"] or "", row["album"] or "")
        if lyrics:
            conn.execute("UPDATE tracks SET lyrics = ? WHERE id = ?", (lyrics, track_id))
            conn.commit()
    conn.close()
    return {"lyrics": lyrics}

@router.put("/api/tracks/{track_id}/lyrics")
def update_track_lyrics(track_id: int, body: dict):
    lyrics = body.get("lyrics", "")
    conn = get_connection()
    conn.execute("UPDATE tracks SET lyrics = ? WHERE id = ?", (lyrics, track_id))
    conn.commit()
    conn.close()
    return {"ok": True}


@router.get("/api/albums")
def list_albums(grouped: Optional[bool] = Query(False)):
    conn = get_connection()
    rows = conn.execute(
        """SELECT album, artist, album_artist, COUNT(*) as track_count,
                  MAX(has_cover) as has_cover, MIN(id) as cover_track_id
           FROM tracks
           WHERE album IS NOT NULL
           GROUP BY album, COALESCE(NULLIF(album_artist,''), artist)
           ORDER BY COALESCE(NULLIF(album_artist,''), artist), album""",
    ).fetchall()
    conn.close()
    albums = [dict(r) for r in rows]
    if not grouped:
        return albums
    groups = {}
    for a in albums:
        base = _base_album_name(a["album"])
        key = (base, a.get("album_artist") or a["artist"])
        if key not in groups:
            groups[key] = {"base_album": base, "artist": key[1], "versions": []}
        groups[key]["versions"].append(a)
    return sorted(groups.values(), key=lambda g: (g["artist"].lower() or "", g["base_album"].lower()))


@router.get("/api/albums/tracks")
def list_album_tracks(album: str = Query(...), artist: Optional[str] = Query(None)):
    conn = get_connection()
    if artist:
        rows = conn.execute(
            "SELECT * FROM tracks WHERE album = ? AND COALESCE(NULLIF(album_artist,''), artist) = ? ORDER BY disc_number, track_number",
            (album, artist),
        ).fetchall()
    else:
        rows = conn.execute(
            "SELECT * FROM tracks WHERE album = ? ORDER BY disc_number, track_number", (album,)
        ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


@router.get("/api/artist-image/{artist_name:path}")
def get_artist_image(artist_name: str):
    import hashlib, json, urllib.request, urllib.parse

    safe_name = artist_name.strip()
    if not safe_name:
        safe_name = "?"

    cache_key = hashlib.md5(safe_name.lower().encode()).hexdigest()[:16]
    for ext in (".jpg", ".png", ".webp"):
        p = COVERS_DIR / f"artist_{cache_key}{ext}"
        if p.exists():
            mt = {"jpg": "image/jpeg", "png": "image/png", "webp": "image/webp"}.get(ext[1:], "image/jpeg")
            return FileResponse(str(p), media_type=mt, headers={"Cache-Control": "public, max-age=86400"})

    try:
        q = urllib.parse.quote(safe_name)
        req = urllib.request.Request(
            f"https://api.deezer.com/search/artist?q={q}&limit=1",
            headers={"User-Agent": "Moidify/1.0"},
        )
        with urllib.request.urlopen(req, timeout=5) as resp:
            data = json.loads(resp.read().decode())
        if data.get("data") and len(data["data"]) > 0:
            picture_url = data["data"][0].get("picture_medium") or data["data"][0].get("picture")
            if picture_url:
                img_req = urllib.request.Request(picture_url, headers={"User-Agent": "Moidify/1.0"})
                with urllib.request.urlopen(img_req, timeout=5) as img_resp:
                    img_data = img_resp.read()
                ext = ".jpg"
                if img_data[:4] == b'\x89PNG':
                    ext = ".png"
                elif img_data[:4] == b'RIFF' and img_data[8:12] == b'WEBP':
                    ext = ".webp"
                cache_path = COVERS_DIR / f"artist_{cache_key}{ext}"
                with open(cache_path, "wb") as f:
                    f.write(img_data)
                mt = {"jpg": "image/jpeg", "png": "image/png", "webp": "image/webp"}.get(ext[1:], "image/jpeg")
                return FileResponse(str(cache_path), media_type=mt, headers={"Cache-Control": "public, max-age=86400"})
    except Exception:
        pass

    h = hashlib.md5(safe_name.encode()).hexdigest()
    hue1 = int(h[:2], 16) % 360
    hue2 = (hue1 + 40) % 360
    initial = safe_name[0].upper() if safe_name else "?"
    svg = f'''<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 200 200">
      <defs><linearGradient id="g" x1="0%" y1="0%" x2="100%" y2="100%">
        <stop offset="0%" style="stop-color:hsl({hue1},60%,35%)"/>
        <stop offset="100%" style="stop-color:hsl({hue2},60%,45%)"/>
      </linearGradient></defs>
      <rect width="200" height="200" rx="40" fill="url(#g)"/>
      <text x="100" y="120" text-anchor="middle" fill="rgba(255,255,255,0.85)" font-size="80" font-family="system-ui,sans-serif" font-weight="600">{initial}</text>
    </svg>'''
    return Response(content=svg, media_type="image/svg+xml", headers={"Cache-Control": "public, max-age=3600"})


@router.get("/api/artists")
def list_artists():
    conn = get_connection()
    rows = conn.execute(
        """SELECT COALESCE(NULLIF(album_artist,''), artist) as artist,
                  COUNT(*) as track_count,
                  COUNT(DISTINCT album) as album_count,
                  MIN(id) as cover_track_id
           FROM tracks
           WHERE artist IS NOT NULL
           GROUP BY COALESCE(NULLIF(album_artist,''), artist)
           ORDER BY artist""",
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


@router.get("/api/artists/tracks")
def list_artist_tracks(artist: str = Query(...)):
    conn = get_connection()
    rows = conn.execute(
        """SELECT * FROM tracks
           WHERE COALESCE(NULLIF(album_artist,''), artist) = ?
           ORDER BY album, disc_number, track_number""", (artist,)
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


@router.get("/api/genres")
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


@router.get("/api/genres/tracks")
def list_genre_tracks(genre: str = Query(...)):
    conn = get_connection()
    rows = conn.execute(
        "SELECT * FROM tracks WHERE genre = ? ORDER BY album, disc_number, track_number", (genre,)
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


@router.get("/api/home")
def home_feed(token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    conn = get_connection()

    recently_played = []
    if user:
        recently_played = conn.execute(
            """SELECT DISTINCT t.* FROM play_history ph
               JOIN tracks t ON ph.track_id = t.id
               GROUP BY t.id
               ORDER BY MAX(ph.played_at) DESC
               LIMIT 20"""
        ).fetchall()

    recommended_albums = conn.execute(
        """SELECT album, COALESCE(NULLIF(album_artist,''), artist) as artist, album_artist,
                  COUNT(*) as track_count, MAX(has_cover) as has_cover, MIN(id) as cover_track_id
           FROM tracks WHERE album IS NOT NULL
           GROUP BY album, COALESCE(NULLIF(album_artist,''), artist)
           ORDER BY RANDOM() LIMIT 12"""
    ).fetchall()

    recommended_tracks = conn.execute(
        "SELECT * FROM tracks ORDER BY RANDOM() LIMIT 20"
    ).fetchall()

    recommended_artists = conn.execute(
        """SELECT COALESCE(NULLIF(album_artist,''), artist) as artist,
                  COUNT(*) as track_count, COUNT(DISTINCT album) as album_count,
                  MIN(id) as cover_track_id
           FROM tracks WHERE artist IS NOT NULL
           GROUP BY COALESCE(NULLIF(album_artist,''), artist)
           ORDER BY RANDOM() LIMIT 12"""
    ).fetchall()

    playlists = []
    if user:
        playlists = conn.execute(
            """SELECT p.*, COUNT(pt.id) as track_count
               FROM playlists p
               LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id
               WHERE p.user_id = ?
               GROUP BY p.id
               ORDER BY RANDOM() LIMIT 6""",
            (user["id"],),
        ).fetchall()

    conn.close()
    return {
        "recently_played": [dict(r) for r in recently_played],
        "recommended_albums": [dict(r) for r in recommended_albums],
        "recommended_tracks": [dict(r) for r in recommended_tracks],
        "recommended_artists": [dict(r) for r in recommended_artists],
        "playlists": [dict(r) for r in playlists],
    }


@router.get("/api/tracks/{track_id}/rating")
def get_track_rating(track_id: int):
    conn = get_connection()
    row = conn.execute("SELECT rating FROM tracks WHERE id = ?", (track_id,)).fetchone()
    conn.close()
    if row is None:
        raise HTTPException(404, "Track not found")
    return {"rating": row["rating"] or 0}


@router.put("/api/tracks/{track_id}/rating")
def set_track_rating(track_id: int, body: RatingBody, token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Login required")
    if body.rating < 0 or body.rating > 5:
        raise HTTPException(400, "Rating must be 0-5")
    conn = get_connection()
    conn.execute("UPDATE tracks SET rating = ? WHERE id = ?", (body.rating, track_id))
    conn.commit()
    conn.close()
    return {"ok": True, "rating": body.rating}
