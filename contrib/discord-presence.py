#!/usr/bin/env python3
"""
Moidify → Discord Rich Presence companion script.

Polls your Moidify server for the most recently played track
and sets it as your Discord status via pypresence (RPC).

Usage:
  pip install pypresence
  python discord-presence.py --url http://your-server:8000

Requires:
  - Discord desktop client running
  - A Discord application with Rich Presence enabled (use your own CLIENT_ID)
  - pypresence library

Steps to get a client ID:
  1. Go to https://discord.com/developers/applications
  2. Create a new application (name it "Moidify")
  3. Copy the Application ID → pass as --client-id
"""

import argparse
import json
import time
import urllib.request
import urllib.error

try:
    from pypresence import Presence
except ImportError:
    print("pypresence not installed. Run: pip install pypresence")
    exit(1)

DEFAULT_CLIENT_ID = "133742069133742069"  # Replace with your own


def fetch_now_playing(server_url: str):
    url = server_url.rstrip("/") + "/api/player/now-playing"
    try:
        resp = urllib.request.urlopen(url, timeout=5)
        return json.loads(resp.read())
    except (urllib.error.URLError, json.JSONDecodeError, OSError) as e:
        return None


def format_duration(seconds):
    if not seconds:
        return "0:00"
    m = int(seconds // 60)
    s = int(seconds % 60)
    return f"{m}:{s:02d}"


def main():
    parser = argparse.ArgumentParser(description="Moidify Discord Rich Presence")
    parser.add_argument("--url", default="http://localhost:8000",
                        help="Moidify server URL (default: http://localhost:8000)")
    parser.add_argument("--client-id", default=DEFAULT_CLIENT_ID,
                        help="Discord Application ID")
    parser.add_argument("--interval", type=int, default=15,
                        help="Polling interval in seconds (default: 15)")
    args = parser.parse_args()

    print(f"Connecting to Discord (client ID: {args.client_id})...")
    try:
        rpc = Presence(args.client_id)
        rpc.connect()
        print("Connected to Discord.")
    except Exception as e:
        print(f"Failed to connect to Discord: {e}")
        print("Make sure Discord desktop is running.")
        return

    last_track_id = None

    print(f"Polling {args.url} every {args.interval}s...")
    try:
        while True:
            data = fetch_now_playing(args.url)
            if data and data.get("id") and data["id"] != last_track_id:
                last_track_id = data["id"]
                title = data.get("title") or "Unknown Track"
                artist = data.get("artist") or "Unknown Artist"
                album = data.get("album")
                duration = data.get("duration")

                details = title
                state = f"by {artist}"
                if album:
                    state += f" • {album}"

                end_time = None
                if duration:
                    end_time = int(time.time()) + int(duration)

                try:
                    rpc.update(
                        details=details[:128],
                        state=state[:128],
                        large_image="moidify_logo",
                        large_text="Moidify",
                        small_image="play",
                        small_text="Playing",
                        end=end_time,
                        buttons=[
                            {"label": "Listen on Moidify", "url": args.url.rstrip("/")}
                        ],
                    )
                    print(f"Now playing: {title} — {artist}")
                except Exception as e:
                    print(f"RPC update error: {e}")

            time.sleep(args.interval)

    except KeyboardInterrupt:
        print("\nDisconnecting...")
        try:
            rpc.clear()
            rpc.close()
        except Exception:
            pass


if __name__ == "__main__":
    main()
