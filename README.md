# Moidify

A self-hosted music server that streams your music collection to any browser. Drop your files in, listen instantly — no accounts required, no algorithms, no ads.

## Features

- **Zero-config streaming** — drop MP3/FLAC/OGG/M4A/WAV into your music folder, it appears instantly
- **Free Spotify model** — anyone on your network can listen without logging in
- **Accounts + playlists** — optional user accounts for playlists, favorites, and session persistence
- **Album/artist browsing** — grid or list view, cover art, metadata from your files
- **Full-text search** — search tracks, albums, and artists with diacritics normalization (type `Beyonce` and find `Beyoncé`)
- **Smart queue** — crossfade, repeat modes, shuffle, sleep timer
- **Drag-and-drop playlists** — reorder tracks by dragging
- **Sortable columns** — click any track column header to sort A-Z, Z-A, or by duration
- **Equalizer** — 10-band EQ with presets
- **Lyrics** — fetches lyrics automatically
- **Admin dashboard** — rescan library, manage users, view stats
- **Responsive** — works on desktop and mobile browsers

## Quick Install

```bash
curl -sSL https://raw.githubusercontent.com/Moid-M/moidify/main/install.sh | sudo bash
```

The installer will:
1. Detect your distro and install dependencies (Python, pip, sqlite3)
2. Create a `moidify` system user
3. Install the app to `/opt/moidify`
4. Set up a Python virtual environment
5. Ask for your music folder location
6. Install a systemd service on port 8000
7. Start the server immediately

After installation, open **http://your-server-ip:8000** in your browser and drop music into the configured folder.

## Manual Install

```bash
git clone https://github.com/Moid-M/moidify.git
cd moidify
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
python3 server.py
```

Then open http://localhost:8000.

## Configuration

### Environment variables

| Variable | Default | Description |
|---|---|---|
| `MOIDIFY_MUSIC_DIR` | `./music` | Path to your music folder |
| `MOIDIFY_COVERS_DIR` | `./covers` | Path for extracted cover art cache |
| `MOIDIFY_DB_PATH` | `./data/music.db` | Path to the SQLite database |

### Config file (installed mode)

When installed via the script, settings live in `/etc/moidify/config.json`:

```json
{
  "music_dir": "/path/to/your/music",
  "covers_dir": "/var/lib/moidify/covers",
  "db_path": "/var/lib/moidify/music.db"
}
```

Change a path and restart: `sudo systemctl restart moidify`

## Updating

```bash
sudo /opt/moidify/update.sh
```

This pulls the latest code from GitHub, installs any new Python dependencies, and restarts the service.

## Uninstalling

```bash
sudo /opt/moidify/uninstall.sh
```

## Commands

| Action | Command |
|---|---|
| Start | `sudo systemctl start moidify` |
| Stop | `sudo systemctl stop moidify` |
| Restart | `sudo systemctl restart moidify` |
| Status | `sudo systemctl status moidify` |
| Logs | `journalctl -u moidify.service -f` |

## Development

```bash
git clone https://github.com/Moid-M/moidify.git
cd moidify
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
python3 server.py
```

The server watches your music folder for changes and picks up new files automatically. No rebuild step needed — the frontend is vanilla JS.

## Tech Stack

- **Backend:** Python + FastAPI + uvicorn
- **Frontend:** Vanilla JavaScript (no framework)
- **Database:** SQLite
- **Metadata:** Mutagen

## License

MIT
