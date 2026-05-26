<div align="center">
  <br>
  <img src="https://raw.githubusercontent.com/Moid-M/moidify/main/static/placeholder-cover.svg" width="80" alt="Moidify">
  <h1 align="center">🎵 Moidify</h1>
  <p align="center">
    <strong>Your music. Anywhere. No strings attached.</strong>
    <br>
    A self-hosted music server that streams your collection to any browser.
    <br>
    Drop files. Listen instantly. No accounts required. No algorithms. No ads.
  </p>
  <p align="center">
    <a href="#-quick-install">🚀 Quick Install</a>
    ·
    <a href="#-features">✨ Features</a>
    ·
    <a href="#-commands">⚙️ Commands</a>
    ·
    <a href="#-configuration">🔧 Configuration</a>
    ·
    <a href="#-development">🛠️ Development</a>
  </p>
  <p align="center">
    <img src="https://img.shields.io/badge/python-3.9%2B-blue?style=flat-square&logo=python">
    <img src="https://img.shields.io/badge/license-MIT-green?style=flat-square">
    <img src="https://img.shields.io/badge/status-stable-brightgreen?style=flat-square">
  </p>
  <br>
</div>

---

## ✨ Features

<table>
<tr>
<td width="50%">

**🎧 Streaming** — MP3, FLAC, OGG, M4A, WAV — everything just works

**🔍 Full-text Search** — search tracks, albums, artists with diacritics support (`Beyonce` → `Beyoncé`)

**📂 Browse by Album / Artist / Genre** — grid or list view with cover art

**📋 Sortable Columns** — click any column header to sort A–Z, Z–A, or by duration

**📝 Playlists** — create, pin, reorder by drag-and-drop

</td>
<td width="50%">

**🎛️ 10-Band Equalizer** — presets included (Rock, Jazz, Dance, Classical…)

**📜 Lyrics** — auto-fetched from LRCLIB, synced scrolling

**⏱️ Sleep Timer** — stop after this track, end of queue, or in X minutes

**🔀 Smart Queue** — crossfade, shuffle, repeat (all/one/off)

**💾 Session Persistence** — close or reload the tab and pick up exactly where you left off (position, queue, shuffle mode all saved)

**💿 Vinyl Animation** — spinning disc with CD hole effect on the queue cover art (toggle in settings)

**🛡️ Admin Dashboard** — rescan library, manage users, view play stats, upload files via drag-and-drop

**🔧 Setup Wizard** — first-run wizard at `/setup` creates admin account and guides configuration

**🐳 Docker Support** — Dockerfile + docker-compose.yml for containerized deployment

**🔗 Shareable Playlists** — generate a public link anyone can open and listen to (no account needed)

**📻 On-the-fly Transcoding** — serve medium (256k), low (128k), or voice (64k Opus) streams via ffmpeg

**🎮 Discord Rich Presence** — companion script shows your currently playing track on Discord

**📱 Responsive** — works on desktop and mobile browsers

**🎚️ Track Rating** — rate songs 1-5 stars from the track list or context menu

</td>
</tr>
</table>

---

## 🚀 Quick Install

One line, works on any Linux distro with systemd:

```bash
curl -sSL https://raw.githubusercontent.com/Moid-M/moidify/main/install.sh | sudo bash
```

<details>
<summary><b>📦 What the installer does (click to expand)</b></summary>
<br>

| Step | What happens |
|---|---|
| 1 | Detects your distro (`apt`/`dnf`/`pacman`/`zypper`) and installs Python, pip, sqlite3 |
| 2 | Creates a `moidify` system user |
| 3 | Copies the app to `/opt/moidify` |
| 4 | Sets up a Python virtual environment |
| 5 | Installs Python dependencies (FastAPI, uvicorn, mutagen, watchdog) |
| 6 | **Asks for port number** (default **8000**) |
| 7 | **Asks for your music folder location** |
| 8 | **Asks to create an admin account** (optional — skip to use the browser setup wizard later) |
| 9 | Writes config to `/etc/moidify/config.json` |
| 10 | Installs a systemd service |
| 11 | Starts the server immediately |
</details>
<br>

> [!TIP]
> After installation, open **http://your-server-ip:8000** in any browser. If no admin account exists yet, you'll be guided through the **setup wizard** at `/setup`. Drop music into your folder — files appear automatically.

> [!NOTE]
> **Moidify is 100% vibecoded.**  
> Every line was written through natural language prompts — no copy-paste, no templates, just pure AI-generated code.
> If something breaks (and it might), please [open an issue](https://github.com/Moid-M/moidify/issues).  
> It'll get fixed, and the AI will learn from it.

---

## 🐳 Docker

```bash
git clone https://github.com/Moid-M/moidify.git
cd moidify
docker compose up -d
```

Then open **http://localhost:8000**. Music goes in `./music`, data in `./data`, covers in `./covers`.

> [!TIP]
> After starting, visit the **setup wizard** at `/setup` to create your admin account.

## 🖥️ Manual Install

For development or non-systemd systems:

```bash
git clone https://github.com/Moid-M/moidify.git
cd moidify
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
python3 server.py
```

Then open **http://localhost:8000**.

---

## ⚙️ Commands

### Service management (installed mode)

| Action | Command |
|---|---|
| ▶️ Start | `sudo systemctl start moidify` |
| ⏹️ Stop | `sudo systemctl stop moidify` |
| 🔄 Restart | `sudo systemctl restart moidify` |
| 📊 Status | `sudo systemctl status moidify` |
| 📜 Logs | `journalctl -u moidify.service -f` |
| 🔄 Update | `sudo /opt/moidify/update.sh` |
| 🗑️ Uninstall | `sudo /opt/moidify/uninstall.sh` |

---

## 🔧 Configuration

Moidify checks three places for settings, in order of priority:

1. **Environment variables** (highest priority)
2. **Config file** at `/etc/moidify/config.json` (installed mode)
3. **Local defaults** (development mode)

### Environment variables

| Variable | Default (dev) | Description |
|---|---|---|
| `MOIDIFY_MUSIC_DIR` | `./music` | Path to your music folder |
| `MOIDIFY_COVERS_DIR` | `./covers` | Cover art cache location |
| `MOIDIFY_DB_PATH` | `./data/music.db` | SQLite database path |
| `MOIDIFY_PORT` | `8000` | Server port (systemd mode uses ExecStart directly) |

### Config file

When installed, settings live in `/etc/moidify/config.json`:

```json
{
  "music_dir": "/path/to/your/music",
  "covers_dir": "/var/lib/moidify/covers",
  "db_path": "/var/lib/moidify/music.db",
  "port": 8000
}
```

> [!NOTE]
> Change a path and restart with `sudo systemctl restart moidify` — your music collection is re-scanned automatically.

---

## 🎮 Discord Rich Presence

Show what you're listening to on your Discord profile:

```bash
pip install pypresence
python3 contrib/discord-presence.py --url http://your-server:8000
```

The script polls Moidify's `/api/player/now-playing` endpoint and updates your Discord status via RPC. Requires the Discord desktop app running and a Discord Application ID (create one at the [Discord Developer Portal](https://discord.com/developers/applications)).

---

## 🛠️ Development

```bash
git clone https://github.com/Moid-M/moidify.git
cd moidify
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
python3 server.py
```

> [!TIP]
> The server uses **watchdog** to monitor your music folder for changes. Drop new files in and they appear instantly — no rescan button needed.

### Project structure

```
moidify/
├── contrib/
│   └── discord-presence.py  # Discord RPC companion script
├── Dockerfile         # Container image
├── docker-compose.yml # Docker orchestration
├── server.py          # FastAPI app (API + streaming)
├── scanner.py         # File scanner + metadata extractor
├── database.py        # SQLite schema + migrations
├── config.py          # Configuration loader
├── install.sh         # System installer script
├── uninstall.sh       # Cleanup script
├── update.sh          # Git-pull updater
├── moidify.service    # Systemd unit file
├── requirements.txt   # Python dependencies
├── static/
│   ├── index.html     # Main frontend
│   ├── setup.html     # First-run setup wizard
│   ├── shared.html    # Public shared playlist page
│   ├── admin.html     # Admin dashboard
│   ├── style.css      # All styles
│   ├── placeholder-cover.svg
│       └── js/
│       ├── state.js   # App state + utility functions
│       ├── icons.js   # SVG icon library (share, copy, check icons)
│       ├── api.js     # API client + auth + favorites
│       ├── i18n.js    # Internationalization (English/German)
│       ├── player.js  # Audio engine + EQ + transcoding quality
│       ├── queue.js   # Queue management + shuffle
│       ├── lyrics.js  # Lyrics fetching + synced display
│       ├── animations.js  # Visual effects (vinyl spin, CD hole, glow)
│       ├── ui.js      # Modals, settings, context menu, keyboard shortcuts
│       ├── views.js   # All page renderers (albums, artists, playlists, search)
│       └── app.js     # Event binding + session persistence + init
└── music/             # Your music goes here (local dev)
```

---

## 📸 Screenshots

<details>
<summary><b>Click to expand</b></summary>
<br>

> *Screenshots coming soon. The project is actively developed — expect visual polish in future releases.*

</details>

---

## 🧩 Tech Stack

| Layer | Technology |
|---|---|
| **Backend** | Python + [FastAPI](https://fastapi.tiangolo.com/) + uvicorn |
| **Frontend** | Vanilla JavaScript (no framework, no build step) |
| **Database** | SQLite (via `sqlite3`) |
| **Metadata** | [Mutagen](https://mutagen.readthedocs.io/) |
| **File watching** | [watchdog](https://github.com/gorakhargosh/watchdog) |

---

## 📄 License

This project is **open source** — you are free to use, modify, share, sell, or do absolutely anything you want with it. No strings attached.

[MIT](LICENSE)

---

<div align="center">
  <p>Made with ❤️ + 🤖 for people who love their music collection.</p>
  <p>
    <a href="https://github.com/Moid-M/moidify/issues">🐛 Report a bug</a>
    ·
    <a href="https://github.com/Moid-M/moidify/issues">💡 Request a feature</a>
  </p>
</div>
