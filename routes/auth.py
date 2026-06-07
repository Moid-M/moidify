from fastapi import APIRouter, HTTPException, Header
from typing import Optional

from database import get_connection
from routes.deps import (
    _hash_password, _create_session, _get_user_from_token, _validate_password,
    _check_account_locked, _increment_failed_attempts, _reset_failed_attempts,
    _is_registration_open, check_rate_limit, _token_hash,
    RegisterBody, LoginBody, SetupInitBody, ChangeOwnPasswordBody,
)

router = APIRouter(tags=["auth"])


@router.get("/api/version")
def get_version():
    from routes.deps import APP_VERSION
    return {"version": APP_VERSION}


@router.post("/api/auth/register")
def register(body: RegisterBody):
    if not _is_registration_open():
        raise HTTPException(403, "Registration is disabled")
    if not check_rate_limit(f"register:{body.username}", max_attempts=3, window_seconds=3600):
        raise HTTPException(429, "Too many registration attempts. Try again later.")
    _validate_password(body.password)
    if len(body.username) < 1:
        raise HTTPException(400, "Username is required")
    if body.email and body.email.strip():
        import re
        if not re.match(r'^[^@\s]+@[^@\s]+\.[^@\s]+$', body.email.strip()):
            raise HTTPException(400, "Invalid email format")
    conn = get_connection()
    existing = conn.execute(
        "SELECT id FROM users WHERE username = ?", (body.username,)
    ).fetchone()
    if existing:
        conn.close()
        raise HTTPException(400, "Username already taken")
    pwd_hash, salt = _hash_password(body.password)
    conn.execute(
        "INSERT INTO users (username, email, password_hash, salt) VALUES (?, ?, ?, ?)",
        (body.username, body.email, pwd_hash, salt),
    )
    conn.commit()
    conn.close()
    return {"ok": True}


@router.post("/api/auth/login")
def login(body: LoginBody):
    if not check_rate_limit(f"login:{body.username}", max_attempts=10, window_seconds=900):
        raise HTTPException(429, "Too many login attempts. Try again later.")
    _check_account_locked(body.username)
    conn = get_connection()
    row = conn.execute(
        "SELECT * FROM users WHERE username = ?", (body.username,)
    ).fetchone()
    conn.close()
    if row is None or not row["salt"]:
        raise HTTPException(401, "Invalid credentials")
    pwd_hash, _ = _hash_password(body.password, row["salt"])
    if pwd_hash != row["password_hash"]:
        _increment_failed_attempts(body.username)
        raise HTTPException(401, "Invalid credentials")
    _reset_failed_attempts(body.username)
    token = _create_session(row["id"])
    return {
        "token": token,
        "user": {"id": row["id"], "username": row["username"], "email": row["email"]},
    }


@router.get("/api/auth/me")
def me(token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if user is None:
        raise HTTPException(401, "Not logged in")
    return user


@router.post("/api/auth/password")
def change_own_password(body: ChangeOwnPasswordBody, token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if not user:
        raise HTTPException(401, "Not logged in")
    conn = get_connection()
    row = conn.execute(
        "SELECT * FROM users WHERE id = ?", (user["id"],)
    ).fetchone()
    if not row:
        conn.close()
        raise HTTPException(404, "User not found")
    current_hash, _ = _hash_password(body.current_password, row["salt"])
    if current_hash != row["password_hash"]:
        conn.close()
        raise HTTPException(400, "Current password is incorrect")
    _validate_password(body.new_password)
    new_hash, new_salt = _hash_password(body.new_password)
    conn.execute(
        "UPDATE users SET password_hash = ?, salt = ? WHERE id = ?",
        (new_hash, new_salt, user["id"]),
    )
    # Revoke all other sessions
    conn.execute(
        "DELETE FROM sessions WHERE user_id = ? AND token_hash != ?",
        (user["id"], _token_hash(token)),
    )
    conn.commit()
    conn.close()
    return {"ok": True}


@router.post("/api/auth/logout")
def logout(token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if not user:
        raise HTTPException(401, "Not logged in")
    conn = get_connection()
    conn.execute("DELETE FROM sessions WHERE token_hash = ?", (_token_hash(token),))
    conn.commit()
    conn.close()
    return {"ok": True}


@router.post("/api/auth/logout-all")
def logout_all(token: Optional[str] = Header(None)):
    user = _get_user_from_token(token)
    if not user:
        raise HTTPException(401, "Not logged in")
    conn = get_connection()
    conn.execute("DELETE FROM sessions WHERE user_id = ?", (user["id"],))
    conn.commit()
    conn.close()
    return {"ok": True, "message": "All sessions revoked"}


@router.get("/api/setup/status")
def setup_status():
    conn = get_connection()
    admin_count = conn.execute("SELECT COUNT(*) FROM users WHERE is_admin = 1").fetchone()[0]
    conn.close()
    return {"setup_needed": admin_count == 0, "has_admin": admin_count > 0}


@router.post("/api/setup/init")
def setup_init(body: SetupInitBody):
    _validate_password(body.password)
    conn = get_connection()
    conn.execute("BEGIN IMMEDIATE")
    admin_count = conn.execute("SELECT COUNT(*) FROM users WHERE is_admin = 1").fetchone()[0]
    if admin_count > 0:
        conn.close()
        raise HTTPException(400, "Setup already completed")
    existing = conn.execute("SELECT id FROM users WHERE username = ?", (body.username,)).fetchone()
    if existing:
        conn.close()
        raise HTTPException(400, "Username already taken")
    if body.music_dir:
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)",
            ("music_dir", body.music_dir),
        )
    if body.max_upload_size is not None:
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)",
            ("max_upload_size", str(body.max_upload_size)),
        )
    pwd_hash, salt = _hash_password(body.password)
    cursor = conn.execute(
        "INSERT INTO users (username, password_hash, salt, is_admin) VALUES (?, ?, ?, 1)",
        (body.username, pwd_hash, salt),
    )
    user_id = cursor.lastrowid
    conn.commit()
    conn.close()
    token = _create_session(user_id)
    return {"token": token, "user": {"id": user_id, "username": body.username}}
