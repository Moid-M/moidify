import hashlib
import json
import time
import urllib.request
import urllib.parse
from typing import Optional

from fastapi import APIRouter, HTTPException, Header
from pydantic import BaseModel

from config import BASE_DIR
from database import get_connection
from routes.deps import _get_user_from_token

LASTFM_API_KEY = ""
LASTFM_API_SECRET = ""
LASTFM_API_URL = "https://ws.audioscrobbler.com/2.0/"

router = APIRouter(prefix="/api/lastfm", tags=["lastfm"])


class LastfmConnectBody(BaseModel):
    username: str
    password: str


class LastfmNowPlayingBody(BaseModel):
    artist: str
    track: str
    album: str = ""
    duration: int = 0


class LastfmScrobbleBody(BaseModel):
    artist: str
    track: str
    album: str = ""
    timestamp: int = 0
    duration: int = 0


def _load_keys():
    global LASTFM_API_KEY, LASTFM_API_SECRET
    if LASTFM_API_KEY:
        return
    try:
        from config import INSTALLED_CONFIG
        cfg = json.loads(INSTALLED_CONFIG.read_text())
        LASTFM_API_KEY = cfg.get("lastfm_api_key", "")
        LASTFM_API_SECRET = cfg.get("lastfm_api_secret", "")
    except Exception:
        pass
    if not LASTFM_API_KEY:
        try:
            from config import LASTFM_KEY, LASTFM_SECRET
            LASTFM_API_KEY = LASTFM_KEY
            LASTFM_API_SECRET = LASTFM_SECRET
        except ImportError:
            pass


def _sign(params: dict) -> str:
    _load_keys()
    raw = "".join(f"{k}{params[k]}" for k in sorted(params)) + LASTFM_API_SECRET
    return hashlib.md5(raw.encode()).hexdigest()


def _lastfm_post(method: str, params: dict, sk: str = "") -> dict:
    _load_keys()
    params["api_key"] = LASTFM_API_KEY
    params["method"] = method
    if sk:
        params["sk"] = sk
    params["api_sig"] = _sign(params)
    params["format"] = "json"

    data = urllib.parse.urlencode(params).encode()
    req = urllib.request.Request(LASTFM_API_URL, data=data)
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read().decode())
    except Exception as e:
        return {"error": 999, "message": str(e)}


def _get_session(user_id: int) -> tuple[str, str]:
    conn = get_connection()
    row = conn.execute(
        "SELECT session_key, lastfm_username FROM lastfm_settings WHERE user_id = ?",
        (user_id,),
    ).fetchone()
    conn.close()
    if not row:
        return ("", "")
    return (row["session_key"], row["lastfm_username"])


@router.post("/connect")
def lastfm_connect(body: LastfmConnectBody, token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if not user:
        raise HTTPException(401, "Not authenticated")
    user_id = user["id"]

    _load_keys()
    if not LASTFM_API_KEY or not LASTFM_API_SECRET:
        raise HTTPException(400, "Last.fm API not configured on server")

    result = _lastfm_post("auth.getMobileSession", {
        "username": body.username,
        "password": body.password,
    })
    if result.get("error"):
        raise HTTPException(400, result.get("message", "Last.fm auth failed"))

    session_key = result.get("session", {}).get("key", "")
    if not session_key:
        raise HTTPException(400, "Failed to get session key")

    conn = get_connection()
    conn.execute(
        """INSERT INTO lastfm_settings (user_id, lastfm_username, session_key)
           VALUES (?, ?, ?)
           ON CONFLICT(user_id) DO UPDATE SET lastfm_username=excluded.lastfm_username, session_key=excluded.session_key""",
        (user_id, body.username, session_key),
    )
    conn.commit()
    conn.close()
    return {"ok": True, "username": body.username}


@router.post("/disconnect")
def lastfm_disconnect(token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if not user:
        raise HTTPException(401, "Not authenticated")
    user_id = user["id"]
    conn = get_connection()
    conn.execute("DELETE FROM lastfm_settings WHERE user_id = ?", (user_id,))
    conn.commit()
    conn.close()
    return {"ok": True}


@router.get("/status")
def lastfm_status(token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if not user:
        return {"connected": False}
    sk, username = _get_session(user["id"])
    return {"connected": bool(sk), "username": username}


@router.post("/now-playing")
def lastfm_now_playing(body: LastfmNowPlayingBody, token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if not user:
        raise HTTPException(401, "Not authenticated")
    user_id = user["id"]
    sk, _ = _get_session(user_id)
    if not sk:
        raise HTTPException(400, "Last.fm not connected")

    params = {"artist": body.artist, "track": body.track}
    if body.album:
        params["album"] = body.album
    if body.duration:
        params["duration"] = str(body.duration)

    _lastfm_post("track.updateNowPlaying", params, sk)
    return {"ok": True}


@router.post("/scrobble")
def lastfm_scrobble(body: LastfmScrobbleBody, token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if not user:
        raise HTTPException(401, "Not authenticated")
    user_id = user["id"]
    sk, _ = _get_session(user_id)
    if not sk:
        raise HTTPException(400, "Last.fm not connected")

    ts = body.timestamp or int(time.time())
    params = {
        "artist": body.artist,
        "track": body.track,
        "timestamp": str(ts),
    }
    if body.album:
        params["album"] = body.album
    if body.duration:
        params["duration"] = str(body.duration)

    result = _lastfm_post("track.scrobble", params, sk)
    if result.get("error"):
        raise HTTPException(400, result.get("message", "Scrobble failed"))
    return {"ok": True}
