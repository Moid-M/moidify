import html
import threading
import sys
from pathlib import Path
from contextlib import asynccontextmanager

from fastapi import FastAPI, Request, Response
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import FileResponse
from fastapi.staticfiles import StaticFiles

sys.path.insert(0, str(Path(__file__).parent / "extra-pkgs"))

from config import BASE_DIR, PORT, STATIC_DIR
from database import init_db
from scanner import scan_existing, start_watcher
from routes.admin import start_rescan_scheduler


@asynccontextmanager
async def lifespan(app: FastAPI):
    init_db()
    threading.Thread(target=scan_existing, daemon=True).start()
    start_watcher()
    start_rescan_scheduler()
    yield


app = FastAPI(title="Moidify", lifespan=lifespan)


# Subsonic API
from subsonic import router as subsonic_router
app.include_router(subsonic_router)


# Strip .view suffix for Subsonic clients
@app.middleware("http")
async def subsonic_view_middleware(request: Request, call_next):
    path = request.scope.get("path", "")
    if path.startswith("/rest/") and path.endswith(".view"):
        request.scope["path"] = path[:-5]
    response = await call_next(request)
    return response


# Security headers middleware
@app.middleware("http")
async def security_headers_middleware(request: Request, call_next):
    response = await call_next(request)
    response.headers["X-Content-Type-Options"] = "nosniff"
    response.headers["X-Frame-Options"] = "DENY"
    response.headers["X-XSS-Protection"] = "1; mode=block"
    response.headers["Referrer-Policy"] = "strict-origin-when-cross-origin"
    response.headers["Permissions-Policy"] = "camera=(), microphone=(), geolocation=()"
    response.headers["Content-Security-Policy"] = "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; media-src 'self' blob:; connect-src 'self' ws: wss:; script-src 'self' 'unsafe-inline';"
    return response


# CORS — restrict to same-origin since this is a self-hosted app
# For external access, users configure their reverse proxy
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["*"],
    allow_headers=["*"],
)


# Include all route modules
from routes.auth import router as auth_router
from routes.tracks import router as tracks_router
from routes.streaming import router as streaming_router
from routes.playlists import router as playlists_router
from routes.admin import router as admin_router
from routes.player import router as player_router
from routes.lastfm import router as lastfm_router

app.include_router(auth_router)
app.include_router(tracks_router)
app.include_router(streaming_router)
app.include_router(playlists_router)
app.include_router(admin_router)
app.include_router(player_router)
app.include_router(lastfm_router)


# Health check for Docker/reverse proxy
@app.get("/health")
def health():
    return {"status": "ok"}


# Static pages
@app.get("/setup")
def setup_page():
    from database import get_connection
    conn = get_connection()
    has_admin = conn.execute("SELECT COUNT(*) FROM users WHERE is_admin = 1").fetchone()[0] > 0
    conn.close()
    if has_admin:
        return Response(
            "<html><body><script>window.location.href='/'</script></body></html>",
            media_type="text/html",
        )
    return FileResponse(str(STATIC_DIR / "setup.html"))

@app.get("/admin")
def admin_page():
    return FileResponse(str(STATIC_DIR / "admin.html"))

@app.get("/")
def index():
    return FileResponse(str(STATIC_DIR / "index.html"))

@app.get("/s/{token}")
def shared_playlist_page(token: str):
    return FileResponse(str(STATIC_DIR / "shared.html"))

@app.get("/a/{token}")
def shared_album_page(token: str):
    return FileResponse(str(STATIC_DIR / "shared-album.html"))

@app.get("/track/{track_id}")
def shared_track_page(track_id: int, request: Request):
    
    from database import get_connection
    conn = get_connection()
    row = conn.execute(
        "SELECT id, title, artist, album, duration FROM tracks WHERE id = ?",
        (track_id,),
    ).fetchone()
    conn.close()
    if not row:
        return Response(
            "<html><body><h1>Track not found</h1></body></html>",
            media_type="text/html", status_code=404,
        )
    title = html.escape(row["title"] or "Unknown")
    artist = html.escape(row["artist"] or "Unknown Artist")
    album = html.escape(row["album"] or "")
    duration = row["duration"] or 0
    dur_str = f"{int(duration//60)}:{int(duration%60):02d}" if duration else "?"
    cover_url = f"{request.base_url}api/cover/{track_id}"
    stream_url = f"{request.base_url}api/stream/{track_id}?quality=low"
    app_url = f"{request.base_url}?track={track_id}"
    page = f"""<!DOCTYPE html>
<html>
<head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1.0">
<title>{title} — {artist} | Moidify</title>
<meta property="og:title" content="{title} — {artist}">
<meta property="og:description" content="Listen to {title} by {artist} on Moidify">
<meta property="og:image" content="{cover_url}">
<meta property="og:url" content="{app_url}">
<meta property="og:type" content="music.song">
<meta property="og:site_name" content="Moidify">
<meta name="theme-color" content="#a855f7">
<style>
*,*::before,*::after{{box-sizing:border-box;margin:0;padding:0}}
:root{{--bg:#0d0d0d;--bg-el:#131313;--text:#fff;--text2:#b3b3b3;--text3:#6a6a6a;--accent:#a855f7}}
body{{background:var(--bg);color:var(--text);font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;min-height:100vh;display:flex;align-items:center;justify-content:center;padding:20px}}
.wrap{{max-width:400px;width:100%;text-align:center}}
img{{width:200px;height:200px;border-radius:16px;object-fit:cover;margin-bottom:20px;box-shadow:0 8px 32px rgba(0,0,0,0.5)}}
h1{{font-size:22px;margin-bottom:4px}}
.artist{{color:var(--text2);font-size:15px;margin-bottom:4px}}
.meta{{color:var(--text3);font-size:13px;margin-bottom:24px}}
.controls{{display:flex;align-items:center;justify-content:center;gap:16px;margin-bottom:20px}}
.play-btn{{width:56px;height:56px;border-radius:50%;border:none;background:var(--accent);color:#000;cursor:pointer;display:flex;align-items:center;justify-content:center;transition:transform .15s}}
.play-btn:hover{{transform:scale(1.08)}}
.play-btn svg{{width:24px;height:24px}}
.progress-wrap{{width:100%;display:flex;align-items:center;gap:10px;font-size:12px;color:var(--text3)}}
progress{{flex:1;height:4px;border-radius:2px;appearance:none;background:var(--bg-el)}}
progress::-webkit-progress-bar{{background:var(--bg-el);border-radius:2px}}
progress::-webkit-progress-value{{background:var(--accent);border-radius:2px}}
progress::-moz-progress-bar{{background:var(--accent);border-radius:2px}}
.footer{{margin-top:24px;font-size:13px;color:var(--text3)}}
.footer a{{color:var(--accent);text-decoration:none}}
</style>
</head>
<body>
<div class="wrap">
  <img id="cover" src="{cover_url}" alt="Cover">
  <h1>{title}</h1>
  <div class="artist">{artist}</div>
  <div class="meta">{album}{' · '+dur_str if album else dur_str}</div>
  <div class="controls">
    <button class="play-btn" id="play-btn" onclick="togglePlay()">
      <svg viewBox="0 0 24 24" fill="currentColor" id="play-icon"><polygon points="5,3 19,12 5,21"/></svg>
    </button>
  </div>
  <div class="progress-wrap">
    <span id="current">0:00</span>
    <progress id="progress" value="0" max="100"></progress>
    <span id="total">{dur_str}</span>
  </div>
  <div class="footer">
    <a href="{app_url}">Open in Moidify</a> · Streamed via <a href="https://github.com/Moid-M/moidify">Moidify</a>
  </div>
</div>
<script>
var audio = new Audio('{stream_url}');
var playing = false;
function togglePlay() {{
  if (playing) {{ audio.pause(); }} else {{ audio.play(); }}
}}
audio.addEventListener('play', function() {{
  playing = true;
  document.getElementById('play-icon').innerHTML = '<rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/>';
}});
audio.addEventListener('pause', function() {{
  playing = false;
  document.getElementById('play-icon').innerHTML = '<polygon points="5,3 19,12 5,21"/>';
}});
audio.addEventListener('timeupdate', function() {{
  if (audio.duration) {{
    var pct = (audio.currentTime / audio.duration) * 100;
    document.getElementById('progress').value = pct;
    var m = Math.floor(audio.currentTime / 60);
    var s = Math.floor(audio.currentTime % 60);
    document.getElementById('current').textContent = m + ':' + (s < 10 ? '0' : '') + s;
  }}
}});
audio.addEventListener('ended', function() {{
  playing = false;
  document.getElementById('play-icon').innerHTML = '<polygon points="5,3 19,12 5,21"/>';
  document.getElementById('progress').value = 0;
  document.getElementById('current').textContent = '0:00';
}});
</script>
</body>
</html>"""
    return Response(page, media_type="text/html")


# Now playing
@app.get("/api/player/now-playing")
def now_playing():
    from database import get_connection
    conn = get_connection()
    row = conn.execute(
        """SELECT t.id, t.title, t.artist, t.album, t.duration
           FROM play_history ph JOIN tracks t ON ph.track_id = t.id
           ORDER BY ph.played_at DESC LIMIT 1"""
    ).fetchone()
    conn.close()
    if row:
        return dict(row)
    return {"id": None, "title": None, "artist": None, "album": None, "duration": None}


# Static files
app.mount("/static", StaticFiles(directory=str(STATIC_DIR)), name="static")


# Main
if __name__ == "__main__":
    import uvicorn
    uvicorn.run("server:app", host="0.0.0.0", port=PORT, reload=False)
