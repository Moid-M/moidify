import json
import os
import sys
import tempfile
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent.parent))

from config import BASE_DIR, STATIC_DIR
from database import init_db, get_connection, DB_PATH as _ORIG_DB_PATH
from server import app
from fastapi.testclient import TestClient
import database
import config


@pytest.fixture(autouse=True)
def test_db():
    tmp = tempfile.NamedTemporaryFile(suffix=".db", delete=False)
    tmp.close()
    os.environ["MOIDIFY_DB_PATH"] = tmp.name
    os.environ["MOIDIFY_MUSIC_DIR"] = tempfile.mkdtemp()
    os.environ["MOIDIFY_COVERS_DIR"] = tempfile.mkdtemp()
    # Patch DB_PATH at runtime since config.py already imported it at import time
    new_path = Path(tmp.name)
    config.DB_PATH = new_path
    database.DB_PATH = new_path
    init_db()
    yield
    os.unlink(tmp.name)


@pytest.fixture
def client():
    return TestClient(app)


@pytest.fixture
def auth_token(client):
    r = client.post("/api/setup/init", json={
        "username": "admin",
        "password": "TestPass123!",
    })
    data = r.json()
    assert "token" in data, f"Setup init failed: {data}"
    return data["token"]


def test_health(client):
    r = client.get("/health")
    assert r.status_code == 200
    assert r.json() == {"status": "ok"}


def test_index_page(client):
    r = client.get("/")
    assert r.status_code == 200
    assert "text/html" in r.headers["content-type"]


def test_setup_redirect_when_no_admin(client):
    r = client.get("/api/setup/status")
    assert r.status_code == 200
    data = r.json()
    assert data.get("setup_needed") is True


def test_setup_init(client):
    r = client.post("/api/setup/init", json={
        "username": "admin",
        "password": "TestPass123!",
    })
    assert r.status_code == 200
    data = r.json()
    assert "token" in data
    assert data["user"]["username"] == "admin"


def test_login(client, auth_token):
    assert auth_token is not None
    r = client.post("/api/auth/login", json={
        "username": "admin",
        "password": "TestPass123!",
    })
    assert r.status_code == 200
    data = r.json()
    assert "token" in data


def test_login_invalid(client):
    r = client.post("/api/auth/login", json={
        "username": "admin",
        "password": "wrong",
    })
    assert r.status_code == 401


def test_auth_me(client, auth_token):
    r = client.get("/api/auth/me", headers={"token": auth_token})
    assert r.status_code == 200
    data = r.json()
    assert data.get("username") == "admin"


def test_auth_me_no_token(client):
    r = client.get("/api/auth/me")
    assert r.status_code == 401


def test_tracks_empty(client, auth_token):
    r = client.get("/api/tracks", headers={"token": auth_token})
    assert r.status_code == 200
    assert r.json() == []


def test_albums_empty(client, auth_token):
    r = client.get("/api/albums", headers={"token": auth_token})
    assert r.status_code == 200
    assert r.json() == []


def test_artists_empty(client, auth_token):
    r = client.get("/api/artists", headers={"token": auth_token})
    assert r.status_code == 200
    assert r.json() == []


def test_genres_empty(client, auth_token):
    r = client.get("/api/genres", headers={"token": auth_token})
    assert r.status_code == 200
    assert r.json() == []


def test_admin_stats_no_auth(client):
    r = client.get("/api/admin/stats")
    assert r.status_code == 401


def test_admin_stats(client, auth_token):
    r = client.get("/api/admin/stats", headers={"token": auth_token})
    assert r.status_code == 200
    data = r.json()
    assert "tracks" in data
    assert "artists" in data
    assert "albums" in data


def test_admin_users(client, auth_token):
    r = client.get("/api/admin/users", headers={"token": auth_token})
    assert r.status_code == 200
    data = r.json()
    assert len(data) >= 1
    assert data[0]["username"] == "admin"


def test_admin_scanner_status(client, auth_token):
    r = client.get("/api/admin/scanner", headers={"token": auth_token})
    assert r.status_code == 200
    data = r.json()
    assert "running" in data
    assert "files_found" in data
    assert "files_imported" in data


def test_admin_dashboard(client, auth_token):
    r = client.get("/api/admin/dashboard", headers={"token": auth_token})
    assert r.status_code == 200
    data = r.json()
    assert "genres" in data
    assert "monthly_adds" in data
    assert "plays_per_day" in data
    assert "disk_by_format" in data


def test_admin_plays(client, auth_token):
    r = client.get("/api/admin/plays", headers={"token": auth_token})
    assert r.status_code == 200
    data = r.json()
    assert "total_plays" in data
    assert "total_listen_time" in data


def test_admin_server_info(client, auth_token):
    r = client.get("/api/admin/server-info", headers={"token": auth_token})
    assert r.status_code == 200
    data = r.json()
    assert "python_version" in data
    assert "platform" in data


def test_admin_db_stats(client, auth_token):
    r = client.get("/api/admin/db/stats", headers={"token": auth_token})
    assert r.status_code == 200
    data = r.json()
    assert "track_count" in data
    assert "user_count" in data


def test_version(client):
    r = client.get("/api/version")
    assert r.status_code == 200
    data = r.json()
    assert "version" in data


def test_static_files(client):
    r = client.get("/static/logo.png")
    assert r.status_code == 200


def test_favorites_no_auth(client):
    r = client.get("/api/favorites")
    assert r.status_code == 401


def test_playlists_empty(client, auth_token):
    r = client.get("/api/playlists", headers={"token": auth_token})
    assert r.status_code == 200
    assert r.json() == []


def test_admin_registration_status(client, auth_token):
    r = client.get("/api/admin/registration-status", headers={"token": auth_token})
    assert r.status_code == 200
    data = r.json()
    assert "registration_open" in data


def test_admin_failed_logins(client, auth_token):
    r = client.get("/api/admin/failed-logins", headers={"token": auth_token})
    assert r.status_code == 200
    assert isinstance(r.json(), list)


def test_admin_listening_trends(client, auth_token):
    r = client.get("/api/admin/listening-trends", headers={"token": auth_token})
    assert r.status_code == 200
    data = r.json()
    assert "by_hour" in data
    assert "by_day" in data


def test_admin_db_integrity(client, auth_token):
    r = client.get("/api/admin/db/integrity", headers={"token": auth_token})
    assert r.status_code == 200
    data = r.json()
    assert "ok" in data


def test_admin_schedule_rescan(client, auth_token):
    r = client.get("/api/admin/schedule-rescan", headers={"token": auth_token})
    assert r.status_code == 200
    data = r.json()
    assert "interval_hours" in data


def _add_track(file_path="/nonexistent/test.mp3", title="Song", artist="Artist",
               album="Album", lyrics=None):
    conn = get_connection()
    conn.execute(
        "INSERT INTO tracks (file_path, title, artist, album, lyrics) VALUES (?,?,?,?,?)",
        (file_path, title, artist, album, lyrics),
    )
    tid = conn.execute("SELECT last_insert_rowid()").fetchone()[0]
    conn.commit()
    conn.close()
    return tid


def test_metadata_patch_updates_track_and_handles_missing_audio_file(client, auth_token):
    tid = _add_track(lyrics="old")
    r = client.patch(
        f"/api/admin/tracks/{tid}/metadata",
        headers={"token": auth_token},
        json={"title": "New Title", "artist": "New Artist", "lyrics": "brand new"},
    )
    assert r.status_code == 200
    assert r.json().get("ok") is True
    conn = get_connection()
    row = conn.execute("SELECT title, artist, lyrics FROM tracks WHERE id = ?", (tid,)).fetchone()
    conn.close()
    assert row["title"] == "New Title"
    assert row["artist"] == "New Artist"
    assert row["lyrics"] == "brand new"


def test_metadata_patch_rejects_unauthenticated(client):
    tid = _add_track()
    r = client.patch(f"/api/admin/tracks/{tid}/metadata", json={"title": "X"})
    assert r.status_code == 401


def test_lyrics_endpoint_prefers_embedded(monkeypatch, client):
    import scanner
    import routes.tracks as tracks
    tid = _add_track(lyrics="cached")
    monkeypatch.setattr(scanner, "extract_metadata", lambda fp: {"lyrics": "embedded line"})
    monkeypatch.setattr(tracks, "_fetch_lyrics_from_lrclib", lambda *a, **k: "live")
    r = client.get(f"/api/tracks/{tid}/lyrics")
    assert r.status_code == 200
    d = r.json()
    assert d["lyrics"] == "embedded line"
    assert d["source"] == "file"


def test_lyrics_endpoint_falls_back_to_cache(monkeypatch, client):
    import scanner
    import routes.tracks as tracks
    tid = _add_track(lyrics="cached line")
    monkeypatch.setattr(scanner, "extract_metadata", lambda fp: None)
    monkeypatch.setattr(tracks, "_fetch_lyrics_from_lrclib", lambda *a, **k: None)
    r = client.get(f"/api/tracks/{tid}/lyrics")
    assert r.status_code == 200
    d = r.json()
    assert d["lyrics"] == "cached line"
    assert d["source"] == "cache"


def test_track_cover_upload(client, auth_token):
    tid = _add_track()
    png = b"\x89PNG\r\n\x1a\n" + b"\x00" * 16
    r = client.post(
        f"/api/admin/tracks/{tid}/cover",
        headers={"token": auth_token},
        files={"file": ("cover.png", png, "image/png")},
    )
    assert r.status_code == 200
    conn = get_connection()
    row = conn.execute("SELECT has_cover, cover_hash FROM tracks WHERE id = ?", (tid,)).fetchone()
    conn.close()
    assert row["has_cover"] == 1
    assert row["cover_hash"]


def test_artist_image_upload(client, auth_token):
    artist = "Some Artist"
    _add_track(artist=artist)
    png = b"\x89PNG\r\n\x1a\n" + b"\x00" * 16
    r = client.post(
        f"/api/admin/artists/{artist}/image",
        headers={"token": auth_token},
        files={"file": ("artist.png", png, "image/png")},
    )
    assert r.status_code == 200
