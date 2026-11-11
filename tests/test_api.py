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
    # Reset cached Fernet so each test uses a key next to its temp DB
    import routes.deps as deps
    deps._fernet = None
    init_db()
    yield
    # Clean up key file alongside the DB
    key = new_path.parent / ".moidify_secret_key"
    if key.exists():
        key.unlink()
    os.unlink(tmp.name)
    deps._fernet = None


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


# ── Security regression tests ──────────────────────────────────────────────


def test_no_admin_autopromote_on_init(client):
    """A non-admin user named 'admin' must NOT be promoted by init_db()."""
    # Create a regular user literally named "admin" via direct DB insert
    # (setup hasn't run, so registration is closed — use the DB directly)
    from routes.deps import _hash_password
    pwd, salt = _hash_password("TestPass123!")
    conn = get_connection()
    conn.execute(
        "INSERT INTO users (username, password_hash, salt, is_admin) VALUES (?, ?, ?, 0)",
        ("admin", pwd, salt),
    )
    conn.commit()
    conn.close()
    # Re-run init_db — the old bug would flip is_admin to 1 here
    init_db()
    conn = get_connection()
    row = conn.execute("SELECT is_admin FROM users WHERE username = 'admin'").fetchone()
    conn.close()
    assert row is not None
    assert row["is_admin"] == 0, "init_db must not auto-promote user 'admin'"


def test_registration_blocked_before_setup(client):
    """Registration must be closed until the setup wizard creates an admin."""
    r = client.post("/api/auth/register", json={
        "username": "sneaky",
        "password": "TestPass123!",
    })
    assert r.status_code == 403


def test_registration_open_after_setup(client, auth_token):
    """After setup, registration respects the (default-open) setting."""
    r = client.post("/api/auth/register", json={
        "username": "alice",
        "password": "TestPass123!",
    })
    assert r.status_code == 200


def test_registration_closed_when_disabled(client, auth_token):
    # Toggle registration off (default after setup is open → first toggle closes)
    r = client.post("/api/admin/registration-toggle", headers={"token": auth_token})
    assert r.status_code == 200
    assert r.json()["registration_open"] is False
    r = client.post("/api/auth/register", json={
        "username": "bob",
        "password": "TestPass123!",
    })
    assert r.status_code == 403


def test_lyrics_put_requires_auth(client, auth_token):
    r = client.put("/api/tracks/1/lyrics", json={"lyrics": "hacked"})
    assert r.status_code == 401


def test_subsonic_token_auth(client, auth_token):
    """Modern Subsonic t=md5(password+salt) auth must succeed."""
    import hashlib
    password = "TestPass123!"
    salt = "c19b2d"
    token = hashlib.md5((password + salt).encode()).hexdigest()
    r = client.get("/rest/ping", params={
        "u": "admin", "t": token, "s": salt,
        "v": "1.16.1", "c": "test", "f": "json",
    })
    assert r.status_code == 200
    data = r.json()
    assert data["subsonic-response"]["status"] == "ok"


def test_subsonic_token_auth_wrong_password(client, auth_token):
    import hashlib
    salt = "c19b2d"
    token = hashlib.md5(("WrongPass999!" + salt).encode()).hexdigest()
    # getMusicFolders requires auth (ping does not)
    r = client.get("/rest/getMusicFolders", params={
        "u": "admin", "t": token, "s": salt,
        "v": "1.16.1", "c": "test", "f": "json",
    })
    assert r.status_code == 200
    data = r.json()
    assert data["subsonic-response"]["status"] == "failed"
    assert data["subsonic-response"]["error"]["code"] == "40"


def test_subsonic_plaintext_auth(client, auth_token):
    r = client.get("/rest/ping", params={
        "u": "admin", "p": "TestPass123!",
        "v": "1.16.1", "c": "test", "f": "json",
    })
    assert r.status_code == 200
    data = r.json()
    assert data["subsonic-response"]["status"] == "ok"


def test_subsonic_hex_password_auth(client, auth_token):
    """Legacy enc:<hex> password auth must work."""
    hex_pw = "TestPass123!".encode().hex()
    r = client.get("/rest/ping", params={
        "u": "admin", "p": f"enc:{hex_pw}",
        "v": "1.16.1", "c": "test", "f": "json",
    })
    assert r.status_code == 200
    data = r.json()
    assert data["subsonic-response"]["status"] == "ok"


def test_password_hash_not_plaintext_in_db(client, auth_token):
    conn = get_connection()
    row = conn.execute("SELECT password_hash, password_encrypted FROM users WHERE username = 'admin'").fetchone()
    conn.close()
    assert row is not None
    # PBKDF2 hash must not contain the password
    assert "TestPass123!" not in row["password_hash"]
    # Encrypted copy exists for Subsonic (and is not plaintext either)
    assert row["password_encrypted"] is not None
    assert "TestPass123!" not in row["password_encrypted"]


def test_stream_requires_no_auth_but_download_does(client, auth_token):
    """Anonymous browsing streams, but ZIP downloads require login."""
    r = client.get("/api/download/album", params={"album": "Nope"})
    assert r.status_code == 401


def test_admin_create_user_stores_subsonic_copy(client, auth_token):
    r = client.post("/api/admin/users", json={
        "username": "carol",
        "password": "TestPass123!",
        "is_admin": False,
    }, headers={"token": auth_token})
    assert r.status_code == 200
    conn = get_connection()
    row = conn.execute(
        "SELECT password_encrypted FROM users WHERE username = 'carol'"
    ).fetchone()
    conn.close()
    assert row is not None
    assert row["password_encrypted"] is not None


def test_album_share_status_requires_auth(client, auth_token):
    """Share tokens must not leak to unauthenticated callers."""
    r = client.get("/api/albums/share-status", params={"album": "SomeAlbum"})
    assert r.status_code == 401
