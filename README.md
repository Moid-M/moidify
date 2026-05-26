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

**🛡️ Admin Dashboard** — rescan library, manage users, view play stats, upload files via drag-and-drop

**🔧 Setup Wizard** — first-run wizard at `/setup` creates admin account and guides configuration

**🐳 Docker Support** — Dockerfile + docker-compose.yml for containerized deployment

**📱 Responsive** — works on desktop and mobile browsers

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
│   ├── admin.html     # Admin dashboard
│   ├── style.css      # All styles
│   ├── placeholder-cover.svg
│   └── js/
│       ├── state.js   # App state + utility functions
│       ├── icons.js   # SVG icon library
│       ├── api.js     # API client + auth
│       ├── player.js  # Audio engine + EQ
│       ├── queue.js   # Queue management + shuffle
│       ├── lyrics.js  # Lyrics fetching + synced display
│       ├── animations.js  # Visual effects
│       ├── ui.js      # Modals, settings, context menu
│       ├── views.js   # All page renderers
│       └── app.js     # Event binding + init
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

[MIT](LICENSE) — Do whatever you want with it.

---

<div align="center">
  <p>Made with ❤️ for people who love their music collection.</p>
  <p>
    <a href="https://github.com/Moid-M/moidify/issues">🐛 Report a bug</a>
    ·
    <a href="https://github.com/Moid-M/moidify/issues">💡 Request a feature</a>
  </p>
</div>
