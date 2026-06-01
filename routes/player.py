import json
import time

from fastapi import APIRouter, HTTPException, Header

from database import get_connection
from routes.deps import _get_user_from_token, PlayerStateBody

router = APIRouter(tags=["player"])


@router.get("/api/player/state")
def get_player_state(token: str = Header(None)):
    user = _get_user_from_token(token)
    if not user:
        raise HTTPException(401, "Not authenticated")
    conn = get_connection()
    row = conn.execute(
        "SELECT queue, current_index, current_time, shuffle, repeat_mode, "
        "playback_speed, volume, shuffle_order, shuffle_index FROM player_state WHERE user_id = ?",
        (user["id"],),
    ).fetchone()
    conn.close()
    if not row:
        return {
            "queue": [],
            "current_index": -1,
            "current_time": 0,
            "shuffle": False,
            "repeat_mode": "off",
            "playback_speed": 1.0,
            "volume": 0.7,
            "shuffle_order": [],
            "shuffle_index": 0,
        }
    return {
        "queue": json.loads(row["queue"]) if row["queue"] else [],
        "current_index": row["current_index"],
        "current_time": row["current_time"],
        "shuffle": bool(row["shuffle"]),
        "repeat_mode": row["repeat_mode"],
        "playback_speed": row["playback_speed"],
        "volume": row["volume"],
        "shuffle_order": json.loads(row["shuffle_order"]) if row["shuffle_order"] else [],
        "shuffle_index": row["shuffle_index"],
    }


@router.put("/api/player/state")
def save_player_state(body: PlayerStateBody, token: str = Header(None)):
    user = _get_user_from_token(token)
    if not user:
        raise HTTPException(401, "Not authenticated")
    conn = get_connection()
    conn.execute(
        """INSERT INTO player_state (user_id, queue, current_index, current_time, shuffle, repeat_mode,
           playback_speed, volume, shuffle_order, shuffle_index, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(user_id) DO UPDATE SET
           queue=excluded.queue, current_index=excluded.current_index,
           current_time=excluded.current_time, shuffle=excluded.shuffle,
           repeat_mode=excluded.repeat_mode, playback_speed=excluded.playback_speed,
           volume=excluded.volume, shuffle_order=excluded.shuffle_order,
           shuffle_index=excluded.shuffle_index, updated_at=excluded.updated_at""",
        (
            user["id"],
            json.dumps(body.queue),
            body.current_index,
            body.current_time,
            1 if body.shuffle else 0,
            body.repeat_mode,
            body.playback_speed,
            body.volume,
            json.dumps(body.shuffle_order),
            body.shuffle_index,
            int(time.time()),
        ),
    )
    conn.commit()
    conn.close()
    return {"ok": True}


@router.delete("/api/player/state")
def clear_player_state(token: str = Header(None)):
    user = _get_user_from_token(token)
    if not user:
        raise HTTPException(401, "Not authenticated")
    conn = get_connection()
    conn.execute("DELETE FROM player_state WHERE user_id = ?", (user["id"],))
    conn.commit()
    conn.close()
    return {"ok": True}
