from fastapi import APIRouter, HTTPException, Header
from typing import Optional

from database import get_connection
from routes.deps import (
    _hash_password, _create_session, _get_user_from_token,
    RegisterBody, LoginBody, SetupInitBody,
)

router = APIRouter(tags=["auth"])


@router.get("/api/version")
def get_version():
    from routes.deps import APP_VERSION
    return {"version": APP_VERSION}


@router.post("/api/auth/register")
def register(body: RegisterBody):
    if len(body.password) < 8:
        raise HTTPException(400, "Password must be at least 8 characters")
    if len(body.username) < 1:
        raise HTTPException(400, "Username is required")
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
    conn = get_connection()
    row = conn.execute(
        "SELECT * FROM users WHERE username = ?", (body.username,)
    ).fetchone()
    conn.close()
    if row is None or not row["salt"]:
        raise HTTPException(401, "Invalid credentials")
    pwd_hash, _ = _hash_password(body.password, row["salt"])
    if pwd_hash != row["password_hash"]:
        raise HTTPException(401, "Invalid credentials")
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


@router.get("/api/setup/status")
def setup_status():
    conn = get_connection()
    admin_count = conn.execute("SELECT COUNT(*) FROM users WHERE is_admin = 1").fetchone()[0]
    conn.close()
    return {"setup_needed": admin_count == 0, "has_admin": admin_count > 0}


@router.post("/api/setup/init")
def setup_init(body: SetupInitBody):
    conn = get_connection()
    admin_count = conn.execute("SELECT COUNT(*) FROM users WHERE is_admin = 1").fetchone()[0]
    if admin_count > 0:
        conn.close()
        raise HTTPException(400, "Setup already completed")
    existing = conn.execute("SELECT id FROM users WHERE username = ?", (body.username,)).fetchone()
    if existing:
        conn.close()
        raise HTTPException(400, "Username already taken")
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
