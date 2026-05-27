import hashlib
import json
import re
from pathlib import Path
from typing import Optional
from xml.etree.ElementTree import Element, SubElement, tostring

from fastapi import APIRouter, Query, Request
from fastapi.responses import Response, FileResponse

from database import get_connection
from config import BASE_DIR, MUSIC_DIR, COVERS_DIR

router = APIRouter(prefix="/rest")


_view_re = re.compile(r"\.view$")


def _strip_view(path: str) -> str:
    return _view_re.sub("", path)

API_VERSION = "1.16.1"


def _xml_response(root_el, format="xml", callback=None):
    root_el.set("xmlns", "http://subsonic.org/restapi")
    xml_bytes = tostring(root_el, encoding="UTF-8", xml_declaration=True)
    if format == "json":
        return Response(content=_xml_to_json(xml_bytes), media_type="application/json")
    if format == "jsonp" and callback:
        return Response(
            content=f"{callback}({_xml_to_json(xml_bytes)})",
            media_type="application/javascript",
        )
    return Response(content=xml_bytes, media_type="application/xml")


def _xml_to_json(xml_bytes):
    import xml.etree.ElementTree as ET
    root = ET.fromstring(xml_bytes)
    def el_to_dict(el):
        d = dict(el.attrib)
        children = list(el)
        if children:
            tags = {}
            for child in children:
                key = child.tag.split("}")[-1] if "}" in child.tag else child.tag
                if key in tags:
                    if not isinstance(tags[key], list):
                        tags[key] = [tags[key]]
                    tags[key].append(el_to_dict(child))
                else:
                    tags[key] = el_to_dict(child)
            # check if children are just text tags
            all_text = all(
                not list(child) and child.text and child.text.strip()
                for child in children
            )
            if all_text:
                d.update(tags)
            else:
                d.update(tags)
        elif el.text and el.text.strip():
            d["value"] = el.text.strip()
        return d

    return json.dumps({"subsonic-response": el_to_dict(root)}, indent=2)


def _ok(params=None):
    root = Element("subsonic-response")
    root.set("status", "ok")
    root.set("version", API_VERSION)
    if params:
        for k, v in params.items():
            if isinstance(v, dict):
                SubElement(root, k, v)
            elif isinstance(v, list):
                parent = SubElement(root, k)
                for item in v:
                    if isinstance(item, dict):
                        child_tag = item.pop("_tag", "child")
                        SubElement(parent, child_tag, item)
                    elif isinstance(item, Element):
                        parent.append(item)
    return root


def _error(code, message, params=None):
    root = Element("subsonic-response")
    root.set("status", "failed")
    root.set("version", API_VERSION)
    SubElement(root, "error", {"code": str(code), "message": message})
    if params:
        for k, v in params.items():
            if isinstance(v, dict):
                SubElement(root, k, v)
    return root


def _authenticate(request: Request):
    u = request.query_params.get("u") or ""
    p = request.query_params.get("p") or ""
    t = request.query_params.get("t") or ""
    s = request.query_params.get("s") or ""
    token = request.query_params.get("token") or ""

    if not u:
        return None

    conn = get_connection()
    row = conn.execute("SELECT * FROM users WHERE username = ?", (u,)).fetchone()
    conn.close()
    if not row:
        return None

    pwd_hash = row["password_hash"]
    salt_db = row["salt"]

    # token auth (modern): t = hex(sha256(password + salt)), s = salt (provided by client)
    if t and s:
        expected = hashlib.md5((pwd_hash + s).encode()).hexdigest()
        if t.lower() == expected:
            return dict(row)

    # token auth via token param (alternative)
    if token and s:
        expected = hashlib.md5((pwd_hash + s).encode()).hexdigest()
        if token.lower() == expected:
            return dict(row)

    # password auth: p = "enc:" + hex(md5(password))  or plaintext
    if p:
        if p.startswith("enc:"):
            given_hash = p[4:].lower()
            # compare with md5 of the stored password_hash
            stored_md5 = hashlib.md5(row["password_hash"].encode()).hexdigest()
            if given_hash == stored_md5:
                return dict(row)
        else:
            # plaintext password - verify against stored hash
            import secrets
            pwd_hash_check = hashlib.pbkdf2_hmac(
                "sha256", p.encode(), salt_db.encode(), 100000
            ).hex()
            if pwd_hash_check == row["password_hash"]:
                return dict(row)

    return None


def _require_auth(request: Request):
    user = _authenticate(request)
    if user is None:
        return _error(40, "Wrong username or password")
    return user


def _track_to_child(track):
    return {
        "id": str(track["id"]),
        "parent": str(track["id"]),
        "title": track["title"] or "Unknown",
        "artist": track["artist"] or "Unknown",
        "album": track["album"] or "Unknown",
        "year": str(track["year"] or ""),
        "genre": track["genre"] or "",
        "size": "0",
        "duration": str(int(track["duration"] or 0)),
        "track": str(track["track_number"] or 1),
        "discNumber": str(track["disc_number"] or 1),
        "type": "music",
        "suffix": Path(track["file_path"]).suffix[1:] if track["file_path"] else "mp3",
        "contentType": "audio/mpeg",
        "isDir": "false",
        "path": track["file_path"] or "",
    }


def _album_to_child(album, artist_name=None):
    return {
        "id": "al_" + hashlib.md5(f"{album['album']}_{album.get('album_artist') or album['artist']}".encode()).hexdigest()[:12],
        "parent": "al_" + hashlib.md5(f"{album['album']}_{album.get('album_artist') or album['artist']}".encode()).hexdigest()[:12],
        "title": album["album"] or "Unknown",
        "artist": album.get("album_artist") or album["artist"] or "Unknown",
        "year": "",
        "genre": "",
        "duration": "0",
        "track": "1",
        "discNumber": "1",
        "type": "album",
        "suffix": "",
        "contentType": "",
        "isDir": "true",
        "path": f"{album['album']}",
        "coverArt": str(album.get("cover_track_id") or album.get("id") or ""),
        "songCount": str(album.get("track_count") or 0),
        "album": album["album"] or "",
    }


def _artist_to_child(artist_name, track_count=0, album_count=0):
    artist_id = "ar_" + hashlib.md5(artist_name.encode()).hexdigest()[:12]
    return {
        "id": artist_id,
        "name": artist_name,
        "albumCount": str(album_count or 0),
        "coverArt": "",
    }


# ─── Endpoints ───────────────────────────────────────────────────────────


@router.api_route("/ping", methods=["GET", "POST"])
def ping(request: Request):
    user = _authenticate(request)
    if user:
        root = _ok()
        return _xml_response(root, request.query_params.get("f", "xml"), request.query_params.get("callback"))
    return _xml_response(_ok(), request.query_params.get("f", "xml"), request.query_params.get("callback"))


@router.api_route("/getLicense", methods=["GET", "POST"])
def get_license(request: Request):
    user = _require_auth(request)
    if isinstance(user, Element):
        return _xml_response(user, request.query_params.get("f", "xml"), request.query_params.get("callback"))
    root = _ok({
        "license": {
            "valid": "true",
            "email": "",
            "licenseExpires": "",
            "trialExpires": "",
            "isTrial": "false",
        }
    })
    return _xml_response(root, request.query_params.get("f", "xml"), request.query_params.get("callback"))


@router.api_route("/getMusicFolders", methods=["GET", "POST"])
def get_music_folders(request: Request):
    user = _require_auth(request)
    if isinstance(user, Element):
        return _xml_response(user, request.query_params.get("f", "xml"), request.query_params.get("callback"))
    root = _ok({
        "musicFolders": {
            "musicFolder": [{"id": "1", "name": "Music"}]
        }
    })
    return _xml_response(root, request.query_params.get("f", "xml"), request.query_params.get("callback"))


@router.api_route("/getIndexes", methods=["GET", "POST"])
def get_indexes(request: Request):
    user = _require_auth(request)
    if isinstance(user, Element):
        return _xml_response(user, request.query_params.get("f", "xml"), request.query_params.get("callback"))

    conn = get_connection()
    rows = conn.execute(
        """SELECT DISTINCT COALESCE(NULLIF(album_artist,''), artist) as artist
           FROM tracks WHERE artist IS NOT NULL
           ORDER BY artist"""
    ).fetchall()
    conn.close()

    indexes = {}
    for r in rows:
        name = r["artist"]
        letter = name[0].upper() if name else "#"
        if letter not in indexes:
            indexes[letter] = []
        indexes[letter].append({"name": name, "id": "ar_" + hashlib.md5(name.encode()).hexdigest()[:12]})

    index_list = []
    for letter in sorted(indexes.keys()):
        index_list.append({
            "_tag": "index",
            "name": letter,
            "artist": indexes[letter],
        })

    root = _ok({
        "indexes": {
            "lastModified": "0",
            "ignoredArticles": "",
            "index": index_list,
        }
    })
    return _xml_response(root, request.query_params.get("f", "xml"), request.query_params.get("callback"))


@router.api_route("/getArtists", methods=["GET", "POST"])
def get_artists(request: Request):
    user = _require_auth(request)
    if isinstance(user, Element):
        return _xml_response(user, request.query_params.get("f", "xml"), request.query_params.get("callback"))

    conn = get_connection()
    rows = conn.execute(
        """SELECT COALESCE(NULLIF(album_artist,''), artist) as artist,
                  COUNT(*) as track_count,
                  COUNT(DISTINCT album) as album_count
           FROM tracks WHERE artist IS NOT NULL
           GROUP BY artist ORDER BY artist"""
    ).fetchall()
    conn.close()

    indexes = {}
    for r in rows:
        name = r["artist"]
        letter = name[0].upper() if name else "#"
        if letter not in indexes:
            indexes[letter] = []
        indexes[letter].append(_artist_to_child(name, r["track_count"], r["album_count"]))

    index_list = []
    for letter in sorted(indexes.keys()):
        index_list.append({
            "_tag": "index",
            "name": letter,
            "artist": indexes[letter],
        })

    root = _ok({
        "artists": {
            "ignoredArticles": "",
            "index": index_list,
        }
    })
    return _xml_response(root, request.query_params.get("f", "xml"), request.query_params.get("callback"))


@router.api_route("/getArtist", methods=["GET", "POST"])
def get_artist(request: Request, id: str = Query(...)):
    user = _require_auth(request)
    if isinstance(user, Element):
        return _xml_response(user, request.query_params.get("f", "xml"), request.query_params.get("callback"))

    # id is like "ar_md5hash" - extract artist name from DB
    conn = get_connection()
    # find the artist by id hash or by name
    rows = conn.execute(
        """SELECT DISTINCT COALESCE(NULLIF(album_artist,''), artist) as artist
           FROM tracks WHERE artist IS NOT NULL
           ORDER BY artist"""
    ).fetchall()
    conn.close()

    artist_name = None
    for r in rows:
        name = r["artist"]
        expected_id = "ar_" + hashlib.md5(name.encode()).hexdigest()[:12]
        if expected_id == id:
            artist_name = name
            break

    if not artist_name:
        return _xml_response(_error(70, "Artist not found"), request.query_params.get("f", "xml"), request.query_params.get("callback"))

    conn = get_connection()
    artist_data = conn.execute(
        """SELECT album, COALESCE(NULLIF(album_artist,''), artist) as artist, album_artist,
                  COUNT(*) as track_count,
                  MAX(has_cover) as has_cover, MIN(id) as cover_track_id
           FROM tracks
           WHERE COALESCE(NULLIF(album_artist,''), artist) = ?
           GROUP BY album, COALESCE(NULLIF(album_artist,''), artist)
           ORDER BY album""",
        (artist_name,),
    ).fetchall()
    conn.close()

    children = []
    for a in artist_data:
        album_id = "al_" + hashlib.md5(f"{a['album']}_{a['artist']}".encode()).hexdigest()[:12]
        children.append({
            "id": album_id,
            "parent": id,
            "title": a["album"] or "Unknown",
            "artist": a["artist"] or "Unknown",
            "isDir": "true",
            "coverArt": str(a["cover_track_id"] or ""),
            "songCount": str(a["track_count"] or 0),
            "duration": "0",
            "created": "",
            "year": "",
            "genre": "",
            "type": "album",
            "suffix": "",
            "contentType": "",
            "path": a["album"] or "",
            "album": a["album"] or "",
        })

    root = _ok({
        "artist": {
            "id": id,
            "name": artist_name,
            "albumCount": str(len(children)),
            "album": children,
        }
    })
    return _xml_response(root, request.query_params.get("f", "xml"), request.query_params.get("callback"))


@router.api_route("/getAlbum", methods=["GET", "POST"])
def get_album(request: Request, id: str = Query(...)):
    user = _require_auth(request)
    if isinstance(user, Element):
        return _xml_response(user, request.query_params.get("f", "xml"), request.query_params.get("callback"))

    conn = get_connection()
    album_data = conn.execute(
        """SELECT album, COALESCE(NULLIF(album_artist,''), artist) as artist,
                  COUNT(*) as track_count, MAX(has_cover) as has_cover,
                  MIN(id) as cover_track_id, MIN(year) as year, MIN(genre) as genre
           FROM tracks WHERE album IS NOT NULL
           GROUP BY album, COALESCE(NULLIF(album_artist,''), artist)""",
    ).fetchall()
    conn.close()

    target_album = None
    for a in album_data:
        album_id = "al_" + hashlib.md5(f"{a['album']}_{a['artist']}".encode()).hexdigest()[:12]
        if album_id == id or str(a.get("cover_track_id")) == id:
            target_album = dict(a)
            break

    if not target_album:
        return _xml_response(_error(70, "Album not found"), request.query_params.get("f", "xml"), request.query_params.get("callback"))

    conn = get_connection()
    tracks = conn.execute(
        """SELECT * FROM tracks
           WHERE album = ? AND COALESCE(NULLIF(album_artist,''), artist) = ?
           ORDER BY disc_number, track_number""",
        (target_album["album"], target_album["artist"]),
    ).fetchall()
    conn.close()

    children = [_track_to_child(dict(t)) for t in tracks]
    album_id = "al_" + hashlib.md5(f"{target_album['album']}_{target_album['artist']}".encode()).hexdigest()[:12]

    root = _ok({
        "album": {
            "id": album_id,
            "name": target_album["album"] or "Unknown",
            "artist": target_album["artist"] or "Unknown",
            "coverArt": str(target_album["cover_track_id"] or ""),
            "songCount": str(len(children)),
            "duration": str(sum(t.get("duration") or 0 for t in tracks)),
            "created": "",
            "year": str(target_album["year"] or ""),
            "genre": target_album["genre"] or "",
            "isDir": "false",
            "type": "album",
            "song": children,
        }
    })
    return _xml_response(root, request.query_params.get("f", "xml"), request.query_params.get("callback"))


@router.api_route("/getSong", methods=["GET", "POST"])
def get_song(request: Request, id: str = Query(...)):
    user = _require_auth(request)
    if isinstance(user, Element):
        return _xml_response(user, request.query_params.get("f", "xml"), request.query_params.get("callback"))

    conn = get_connection()
    try:
        track_id = int(id)
        track = conn.execute("SELECT * FROM tracks WHERE id = ?", (track_id,)).fetchone()
    except ValueError:
        track = None
    conn.close()

    if not track:
        return _xml_response(_error(70, "Song not found"), request.query_params.get("f", "xml"), request.query_params.get("callback"))

    root = _ok({"song": _track_to_child(dict(track))})
    return _xml_response(root, request.query_params.get("f", "xml"), request.query_params.get("callback"))


@router.api_route("/getMusicDirectory", methods=["GET", "POST"])
def get_music_directory(request: Request, id: str = Query(...)):
    user = _require_auth(request)
    if isinstance(user, Element):
        return _xml_response(user, request.query_params.get("f", "xml"), request.query_params.get("callback"))

    conn = get_connection()

    # Check if this is an album ID
    album_data = conn.execute(
        """SELECT album, COALESCE(NULLIF(album_artist,''), artist) as artist,
                  COUNT(*) as track_count, MAX(has_cover) as has_cover,
                  MIN(id) as cover_track_id
           FROM tracks WHERE album IS NOT NULL
           GROUP BY album, COALESCE(NULLIF(album_artist,''), artist)""",
    ).fetchall()

    target_album = None
    for a in album_data:
        album_id = "al_" + hashlib.md5(f"{a['album']}_{a['artist']}".encode()).hexdigest()[:12]
        if album_id == id:
            target_album = dict(a)
            break

    if target_album:
        tracks = conn.execute(
            """SELECT * FROM tracks
               WHERE album = ? AND COALESCE(NULLIF(album_artist,''), artist) = ?
               ORDER BY disc_number, track_number""",
            (target_album["album"], target_album["artist"]),
        ).fetchall()
        conn.close()
        children = [_track_to_child(dict(t)) for t in tracks]
        root = _ok({
            "directory": {
                "id": id,
                "name": target_album["album"] or "Unknown",
                "child": children,
            }
        })
        return _xml_response(root, request.query_params.get("f", "xml"), request.query_params.get("callback"))

    # Check if this is an artist ID
    artists = conn.execute(
        """SELECT DISTINCT COALESCE(NULLIF(album_artist,''), artist) as artist
           FROM tracks WHERE artist IS NOT NULL"""
    ).fetchall()
    for a in artists:
        name = a["artist"]
        artist_id = "ar_" + hashlib.md5(name.encode()).hexdigest()[:12]
        if artist_id == id:
            albums = conn.execute(
                """SELECT album, COALESCE(NULLIF(album_artist,''), artist) as artist,
                          COUNT(*) as track_count, MAX(has_cover) as has_cover,
                          MIN(id) as cover_track_id
                   FROM tracks
                   WHERE COALESCE(NULLIF(album_artist,''), artist) = ?
                   GROUP BY album, COALESCE(NULLIF(album_artist,''), artist)
                   ORDER BY album""",
                (name,),
            ).fetchall()
            conn.close()
            children = []
            for a2 in albums:
                aid = "al_" + hashlib.md5(f"{a2['album']}_{a2['artist']}".encode()).hexdigest()[:12]
                children.append({
                    "id": aid,
                    "parent": id,
                    "title": a2["album"] or "Unknown",
                    "artist": a2["artist"] or "Unknown",
                    "isDir": "true",
                    "coverArt": str(a2["cover_track_id"] or ""),
                    "songCount": str(a2["track_count"] or 0),
                    "type": "album",
                    "path": a2["album"] or "",
                    "album": a2["album"] or "",
                })
            root = _ok({
                "directory": {
                    "id": id,
                    "name": name,
                    "child": children,
                }
            })
            return _xml_response(root, request.query_params.get("f", "xml"), request.query_params.get("callback"))

    conn.close()
    return _xml_response(_error(70, "Directory not found"), request.query_params.get("f", "xml"), request.query_params.get("callback"))


@router.api_route("/stream", methods=["GET", "POST"])
@router.api_route("/download", methods=["GET", "POST"])
def stream(request: Request, id: str = Query(...)):
    user = _require_auth(request)
    if isinstance(user, Element):
        return _xml_response(user, request.query_params.get("f", "xml"), request.query_params.get("callback"))

    conn = get_connection()
    try:
        track_id = int(id)
        track = conn.execute("SELECT file_path FROM tracks WHERE id = ?", (track_id,)).fetchone()
    except ValueError:
        track = None
    conn.close()

    if not track:
        return _xml_response(_error(70, "File not found"), request.query_params.get("f", "xml"), request.query_params.get("callback"))

    path = Path(track["file_path"])
    if not path.exists():
        return _xml_response(_error(70, "File not found on disk"), request.query_params.get("f", "xml"), request.query_params.get("callback"))

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

    return FileResponse(str(path), media_type=media_type, headers={
        "Accept-Ranges": "bytes",
        "Content-Disposition": "inline",
    })


@router.api_route("/getCoverArt", methods=["GET", "POST"])
def get_cover_art(request: Request, id: str = Query(...), size: Optional[int] = Query(None)):
    user = _require_auth(request)
    if isinstance(user, Element):
        return _xml_response(user, request.query_params.get("f", "xml"), request.query_params.get("callback"))

    conn = get_connection()
    try:
        track_id = int(id)
    except ValueError:
        track_id = None

    if track_id:
        row = conn.execute("SELECT cover_hash FROM tracks WHERE id = ?", (track_id,)).fetchone()
        if row and row["cover_hash"]:
            for ext in (".jpg", ".png", ".webp"):
                p = COVERS_DIR / f"{row['cover_hash']}{ext}"
                if p.exists():
                    conn.close()
                    mt = {"jpg": "image/jpeg", "png": "image/png", "webp": "image/webp"}.get(ext[1:], "image/jpeg")
                    return FileResponse(str(p), media_type=mt)
        for ext in (".jpg", ".png", ".webp"):
            p = COVERS_DIR / f"{track_id}{ext}"
            if p.exists():
                conn.close()
                mt = {"jpg": "image/jpeg", "png": "image/png", "webp": "image/webp"}.get(ext[1:], "image/jpeg")
                return FileResponse(str(p), media_type=mt)
    conn.close()
    return Response(status_code=404)


@router.api_route("/getAvatar", methods=["GET", "POST"])
def get_avatar(request: Request):
    return Response(status_code=204)


@router.api_route("/search3", methods=["GET", "POST"])
def search3(request: Request):
    user = _require_auth(request)
    if isinstance(user, Element):
        return _xml_response(user, request.query_params.get("f", "xml"), request.query_params.get("callback"))

    query = request.query_params.get("query", "")
    artist_count = int(request.query_params.get("artistCount", 20))
    album_count = int(request.query_params.get("albumCount", 20))
    song_count = int(request.query_params.get("songCount", 20))

    conn = get_connection()
    like = f"%{query}%"

    artists = conn.execute(
        """SELECT DISTINCT COALESCE(NULLIF(album_artist,''), artist) as artist,
                  COUNT(*) as track_count, COUNT(DISTINCT album) as album_count
           FROM tracks WHERE artist LIKE ? OR album_artist LIKE ?
           GROUP BY artist ORDER BY artist LIMIT ?""",
        (like, like, artist_count),
    ).fetchall()

    albums = conn.execute(
        """SELECT album, COALESCE(NULLIF(album_artist,''), artist) as artist,
                  COUNT(*) as track_count, MAX(has_cover) as has_cover,
                  MIN(id) as cover_track_id
           FROM tracks WHERE album LIKE ?
           GROUP BY album, COALESCE(NULLIF(album_artist,''), artist)
           ORDER BY album LIMIT ?""",
        (like, album_count),
    ).fetchall()

    songs = conn.execute(
        """SELECT * FROM tracks
           WHERE title LIKE ? OR artist LIKE ? OR album LIKE ?
           ORDER BY COALESCE(NULLIF(album_artist,''), artist), album, disc_number, track_number
           LIMIT ?""",
        (like, like, like, song_count),
    ).fetchall()
    conn.close()

    root = _ok({
        "searchResult3": {
            "totalHits": str(len(songs)),
            "artist": [_artist_to_child(r["artist"], r["track_count"], r["album_count"]) for r in artists],
            "album": [_album_to_child(dict(r)) for r in albums],
            "song": [_track_to_child(dict(r)) for r in songs],
        }
    })
    return _xml_response(root, request.query_params.get("f", "xml"), request.query_params.get("callback"))


@router.api_route("/getAlbumList2", methods=["GET", "POST"])
def get_album_list2(request: Request):
    user = _require_auth(request)
    if isinstance(user, Element):
        return _xml_response(user, request.query_params.get("f", "xml"), request.query_params.get("callback"))

    a_type = request.query_params.get("type", "random")
    size = int(request.query_params.get("size", 50))
    offset = int(request.query_params.get("offset", 0))

    conn = get_connection()

    order_clause = "ORDER BY RANDOM()"
    if a_type == "newest":
        order_clause = "ORDER BY created_at DESC"
    elif a_type == "recent":
        order_clause = "ORDER BY created_at DESC"
    elif a_type == "alphabeticalByName":
        order_clause = "ORDER BY album"
    elif a_type == "alphabeticalByArtist":
        order_clause = "ORDER BY artist"
    elif a_type == "frequent":
        order_clause = "ORDER BY play_count DESC"
    elif a_type == "random":
        order_clause = "ORDER BY RANDOM()"

    albums = conn.execute(
        f"""SELECT album, COALESCE(NULLIF(album_artist,''), artist) as artist,
                   COUNT(*) as track_count, MAX(has_cover) as has_cover,
                   MIN(id) as cover_track_id
            FROM tracks WHERE album IS NOT NULL
            GROUP BY album, COALESCE(NULLIF(album_artist,''), artist)
            {order_clause}
            LIMIT ? OFFSET ?""",
        (size, offset),
    ).fetchall()
    conn.close()

    root = _ok({
        "albumList2": {
            "album": [_album_to_child(dict(a)) for a in albums],
        }
    })
    return _xml_response(root, request.query_params.get("f", "xml"), request.query_params.get("callback"))


@router.api_route("/getPlaylists", methods=["GET", "POST"])
def get_playlists(request: Request):
    user = _require_auth(request)
    if isinstance(user, Element):
        return _xml_response(user, request.query_params.get("f", "xml"), request.query_params.get("callback"))

    conn = get_connection()
    rows = conn.execute(
        """SELECT p.*, COUNT(pt.id) as track_count
           FROM playlists p
           LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id
           GROUP BY p.id
           ORDER BY p.name"""
    ).fetchall()
    conn.close()

    playlists = []
    for r in rows:
        playlists.append({
            "id": str(r["id"]),
            "name": r["name"] or "Unnamed",
            "owner": user["username"] if user else "admin",
            "public": "false",
            "songCount": str(r["track_count"] or 0),
            "duration": "0",
            "created": "",
        })

    root = _ok({"playlists": {"playlist": playlists}})
    return _xml_response(root, request.query_params.get("f", "xml"), request.query_params.get("callback"))


@router.api_route("/getPlaylist", methods=["GET", "POST"])
def get_playlist(request: Request, id: str = Query(...)):
    user = _require_auth(request)
    if isinstance(user, Element):
        return _xml_response(user, request.query_params.get("f", "xml"), request.query_params.get("callback"))

    conn = get_connection()
    try:
        playlist_id = int(id)
        playlist = conn.execute("SELECT * FROM playlists WHERE id = ?", (playlist_id,)).fetchone()
    except ValueError:
        playlist = None
    conn.close()

    if not playlist:
        return _xml_response(_error(70, "Playlist not found"), request.query_params.get("f", "xml"), request.query_params.get("callback"))

    conn = get_connection()
    tracks = conn.execute(
        """SELECT t.* FROM tracks t
           JOIN playlist_tracks pt ON pt.track_id = t.id
           WHERE pt.playlist_id = ?
           ORDER BY pt.position""",
        (playlist_id,),
    ).fetchall()
    conn.close()

    children = [_track_to_child(dict(t)) for t in tracks]

    root = _ok({
        "playlist": {
            "id": str(playlist["id"]),
            "name": playlist["name"] or "Unnamed",
            "owner": user["username"] if user else "admin",
            "public": "false",
            "songCount": str(len(children)),
            "duration": str(sum(t.get("duration") or 0 for t in tracks)),
            "created": "",
            "entry": children,
        }
    })
    return _xml_response(root, request.query_params.get("f", "xml"), request.query_params.get("callback"))


@router.api_route("/scrobble", methods=["GET", "POST"])
def scrobble(request: Request):
    user = _require_auth(request)
    if isinstance(user, Element):
        return _xml_response(user, request.query_params.get("f", "xml"), request.query_params.get("callback"))

    track_id_str = request.query_params.get("id") or request.query_params.get("trackId", "")
    try:
        track_id = int(track_id_str)
    except ValueError:
        return _xml_response(_error(10, "Missing track id"), request.query_params.get("f", "xml"), request.query_params.get("callback"))

    conn = get_connection()
    conn.execute("UPDATE tracks SET play_count = play_count + 1 WHERE id = ?", (track_id,))
    conn.execute("INSERT INTO play_history (track_id) VALUES (?)", (track_id,))
    conn.commit()
    conn.close()

    root = _ok()
    return _xml_response(root, request.query_params.get("f", "xml"), request.query_params.get("callback"))


@router.api_route("/star", methods=["GET", "POST"])
def star(request: Request):
    user = _require_auth(request)
    if isinstance(user, Element):
        return _xml_response(user, request.query_params.get("f", "xml"), request.query_params.get("callback"))

    track_id_str = request.query_params.get("id", "")
    try:
        track_id = int(track_id_str)
    except ValueError:
        return _xml_response(_error(10, "Invalid id"), request.query_params.get("f", "xml"), request.query_params.get("callback"))

    conn = get_connection()
    existing = conn.execute(
        "SELECT id FROM favorites WHERE user_id = ? AND track_id = ?",
        (user["id"], track_id),
    ).fetchone()
    if not existing:
        conn.execute(
            "INSERT INTO favorites (user_id, track_id) VALUES (?, ?)",
            (user["id"], track_id),
        )
        conn.commit()
    conn.close()

    root = _ok()
    return _xml_response(root, request.query_params.get("f", "xml"), request.query_params.get("callback"))


@router.api_route("/unstar", methods=["GET", "POST"])
def unstar(request: Request):
    user = _require_auth(request)
    if isinstance(user, Element):
        return _xml_response(user, request.query_params.get("f", "xml"), request.query_params.get("callback"))

    track_id_str = request.query_params.get("id", "")
    try:
        track_id = int(track_id_str)
    except ValueError:
        return _xml_response(_error(10, "Invalid id"), request.query_params.get("f", "xml"), request.query_params.get("callback"))

    conn = get_connection()
    conn.execute("DELETE FROM favorites WHERE user_id = ? AND track_id = ?", (user["id"], track_id))
    conn.commit()
    conn.close()

    root = _ok()
    return _xml_response(root, request.query_params.get("f", "xml"), request.query_params.get("callback"))


@router.api_route("/getStarred", methods=["GET", "POST"])
def get_starred(request: Request):
    user = _require_auth(request)
    if isinstance(user, Element):
        return _xml_response(user, request.query_params.get("f", "xml"), request.query_params.get("callback"))

    conn = get_connection()
    songs = conn.execute(
        """SELECT t.* FROM tracks t
           JOIN favorites f ON f.track_id = t.id
           WHERE f.user_id = ?
           ORDER BY f.added_at DESC""",
        (user["id"],),
    ).fetchall()
    conn.close()

    root = _ok({
        "starred": {
            "song": [_track_to_child(dict(s)) for s in songs],
        }
    })
    return _xml_response(root, request.query_params.get("f", "xml"), request.query_params.get("callback"))


@router.api_route("/getRandomSongs", methods=["GET", "POST"])
def get_random_songs(request: Request):
    user = _require_auth(request)
    if isinstance(user, Element):
        return _xml_response(user, request.query_params.get("f", "xml"), request.query_params.get("callback"))

    size = int(request.query_params.get("size", 10))

    conn = get_connection()
    songs = conn.execute(
        "SELECT * FROM tracks ORDER BY RANDOM() LIMIT ?", (size,)
    ).fetchall()
    conn.close()

    root = _ok({
        "randomSongs": {
            "song": [_track_to_child(dict(s)) for s in songs],
        }
    })
    return _xml_response(root, request.query_params.get("f", "xml"), request.query_params.get("callback"))


@router.api_route("/getNowPlaying", methods=["GET", "POST"])
def get_now_playing(request: Request):
    user = _require_auth(request)
    if isinstance(user, Element):
        return _xml_response(user, request.query_params.get("f", "xml"), request.query_params.get("callback"))

    conn = get_connection()
    row = conn.execute(
        """SELECT t.* FROM play_history ph
           JOIN tracks t ON ph.track_id = t.id
           ORDER BY ph.played_at DESC LIMIT 1"""
    ).fetchone()
    conn.close()

    children = []
    if row:
        child = _track_to_child(dict(row))
        child["username"] = user["username"] if user else "admin"
        child["minutesAgo"] = "0"
        child["playerId"] = "0"
        children.append(child)

    root = _ok({"nowPlaying": {"entry": children}})
    return _xml_response(root, request.query_params.get("f", "xml"), request.query_params.get("callback"))
