<div align="center">
  <br>
  <img src="static/logo.png" width="80" alt="Moidify">
  <h1 align="center">🎵 Moidify</h1>
  <p align="center">
    <strong>Your music. Anywhere. No strings attached.</strong>
    <br>
    A self-hosted music server that streams your collection to any browser.
    <br>
    Drop files. Listen instantly. No accounts required. No algorithms. No ads.
    <br>
    <strong>🔒 No telemetry. No tracking. Everything stays yours.</strong>
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
    <img src="https://img.shields.io/badge/version-2.0-gray?style=flat-square">
    <img src="https://img.shields.io/badge/status-stable-brightgreen?style=flat-square">
  </p>
  <br>
</div>

<div align="center">
  <img src="screenshots/desktop/screenshot-desktop-homepage.png" width="800" alt="Moidify Screenshot">
  <br>
  <br>
</div>

---

## ✨ Features

<table>
<tr>
<td width="50%">

**🎧 Streaming** — MP3, FLAC, OGG, M4A, WAV — everything just works

**🪟 Pop-out Mini Player** — detach playback into a separate always-on-top window (desktop only; mobile uses the persistent bottom bar)

**📂 Browse by Album / Artist / Genre** — grid or list view with cover art

**📝 Playlists** — create, pin, reorder by drag-and-drop

**🎛️ 10-Band Equalizer** — presets included (Rock, Jazz, Dance, Classical…)

**📜 Lyrics** — auto-fetched from LRCLIB, companion `.lrc` file support, synced scrolling; upload lyrics per-track from the admin panel, batch-download all missing lyrics at once

**🪟 Liquid Glass Theme** — frosted glass blur effects on the player bar, modals, overlays, context menus, and hover states. Toggle in Settings → Theme → Liquid Glass

**🛡️ Admin Dashboard** — rescan library, manage users, view play stats, upload files via drag-and-drop, upload `.lrc` lyrics per track, batch-scan all tracks for missing lyrics

**📥 YouTube / SoundCloud Import** — download any audio URL via the admin panel or `moidify download <url>` CLI command (uses yt-dlp, converts to 192kbps MP3)

**🐳 Docker Support** — Dockerfile + docker-compose.yml for containerized deployment

**🔧 Setup Wizard** — first-run wizard at `/setup` creates admin account and guides configuration

</td>
<td width="50%">

**🔗 Shareable Playlists** — generate a public link anyone can open and listen to (no account needed)

**📻 On-the-fly Transcoding** — serve medium (256k), low (128k), or voice (64k Opus) streams via ffmpeg

**🏠 Personalized Home Feed** — recently played, top listened, and randomly recommended tracks on login

**🔌 Subsonic API Compatible** — works with clients like Sonixd, Sublime Music, and DSub (`/rest/getArtists`, `/rest/getAlbum`, `/rest/stream`, scrobbling, star/unstar, and more)

**🎮 Discord Rich Presence** — companion script shows your currently playing track on Discord

**📱 Responsive** — works on desktop and mobile browsers

**🔒 No Telemetry** — zero tracking, zero external calls (except optional LRCLIB for lyrics), everything stays on your server

**👤 Dedicated System User** — runs as `moidify` user, not root; isolated and sandboxed by design

**🔐 Optional Auth** — anonymous browsing by default, opt-in user accounts with admin roles; no sign-up wall

</td>
</tr>
</table>

---

## 🚀 Installation

<details>
<summary><b>Quick Install — one-liner (Linux with systemd)</b></summary>
<br>

```bash
curl -sSL https://raw.githubusercontent.com/Moid-M/moidify/main/install.sh | sudo bash
```

<details>
<summary><b>📦 What the installer does</b></summary>
<br>

| Step | What happens |
|---|---|
| 1 | Detects your distro (`apt`/`dnf`/`pacman`/`zypper`) and installs Python, pip, sqlite3 |
| 2 | Creates a `moidify` system user |
| 3 | Copies the app to `/opt/moidify` |
| 4 | Sets up a Python virtual environment |
| 5 | Installs Python dependencies (FastAPI, uvicorn, mutagen, watchdog) |
| 6 | **Asks for max upload size** (default **2.5 GB**) |
| 7 | **Asks for port number** (default **8000**) |
| 8 | **Asks for your music folder location** |
| 9 | **Asks to create an admin account** (optional — skip to use the browser setup wizard later) |
| 10 | Writes config to `/etc/moidify/config.json` |
| 11 | Installs a systemd service |
| 12 | Starts the server immediately |

</details>

> [!TIP]
> After installation, open **http://your-server-ip:8000** in any browser. If no admin account exists yet, you'll be guided through the **setup wizard** at `/setup`. Drop music into your folder — files appear automatically.

> [!NOTE]
> **Moidify is built with AI-assisted coding.**  
> Most of the code was written through natural language prompts rather than manual typing.  
> If something feels off, please [open an issue](https://github.com/Moid-M/moidify/issues) — it helps make things better.

</details>

<details>
<summary><b>🐳 Docker</b></summary>
<br>

```bash
git clone https://github.com/Moid-M/moidify.git
cd moidify
docker compose up -d
```

Then open **http://localhost:8000**. Music goes in `./music`, data in `./data`, covers in `./covers`.

> [!TIP]
> After starting, visit the **setup wizard** at `/setup` to create your admin account.

</details>

<details>
<summary><b>🖥️ Manual Install (dev / non-systemd)</b></summary>
<br>

```bash
git clone https://github.com/Moid-M/moidify.git
cd moidify
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
python3 server.py
```

Then open **http://localhost:8000**.

</details>

---

## ⚙️ Commands

### CLI (`moidify`)

After install, a `moidify` CLI is available globally:

<details>
<summary><b>⚙️ CLI commands (click to expand)</b></summary>
<br>

```bash
moidify help          Show this help
moidify start         Start the service
moidify stop          Stop the service
moidify restart       Restart the service
moidify reload        Reload config and rescan library
moidify status        Show service status
moidify enable        Enable service on boot
moidify disable       Disable service on boot
moidify logs          Tail server logs
moidify config        Print current configuration
moidify version       Print version
moidify url           Print server URL
moidify update        Update to latest version
moidify download <url>  Download and import audio from YouTube/SoundCloud/etc
```

</details>

<details>
<summary><b>🛠️ Service management (click to expand)</b></summary>
<br>

| Action | Command |
|---|---|
| ▶️ Start | `sudo systemctl start moidify` |
| ⏹️ Stop | `sudo systemctl stop moidify` |
| 🔄 Restart | `sudo systemctl restart moidify` |
| 📊 Status | `sudo systemctl status moidify` |
| 📜 Logs | `journalctl -u moidify.service -f` |
| 🔄 Update | `sudo /opt/moidify/update.sh` |
| 🗑️ Uninstall | `sudo /opt/moidify/uninstall.sh` |

</details>

---

## 🔧 Configuration

<details>
<summary><b>Click to expand</b></summary>
<br>

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
| `MOIDIFY_MAX_UPLOAD_SIZE` | `2684354560` (2.5 GB) | Max total upload size in bytes |

### Config file

When installed, settings live in `/etc/moidify/config.json`:

```json
{
  "music_dir": "/path/to/your/music",
  "covers_dir": "/var/lib/moidify/covers",
  "db_path": "/var/lib/moidify/music.db",
  "port": 8000,
  "max_upload_size": 2684354560,
  "lastfm_api_key": "your_lastfm_api_key",
  "lastfm_api_secret": "your_lastfm_shared_secret"
}
```

> [!NOTE]
> Change a path and restart with `sudo systemctl restart moidify` — your music collection is re-scanned automatically.

</details>

---

## 📊 Minimum Requirements & Performance

Moidify is designed to run on modest hardware — a **Raspberry Pi 4 (2 GB)** or any small x86 mini PC handles it comfortably.

| Resource | Idle | Playing + Transcoding | Scanning |
|---|---|---|---|
| **RAM** | 50–80 MB | 80–150 MB | 100–250 MB |
| **CPU** | < 1% | 2–8% (transcoding: 15–40% on Pi 4) | 30–60% (1 core) |
| **Disk** | ~50 MB (app + DB) | negligible | DB grows ~1–2 MB per 10,000 tracks |

### Lightweight by design

- **No JavaScript framework** on the frontend — vanilla HTML/CSS/JS, zero megabytes of node_modules
- **No external database daemon** — SQLite with WAL mode, single file, no background worker
- **No queue worker, no Redis, no message broker** — everything runs in-process
- **Single Python process** — uvicorn with one worker is all you need

### Scan speed

| Storage | Tracks / second | 50,000 tracks |
|---|---|---|
| NVMe SSD | ~3,000 | ~17 s |
| SATA SSD | ~2,000 | ~25 s |
| Pi 4 (USB 3.0 SSD) | ~1,000 | ~50 s |
| Pi 4 (microSD) | ~300 | ~2.8 min |
| HDD (7200 RPM) | ~500 | ~1.7 min |

> Scanning is single-threaded (Python GIL). Each file is opened, tagged with mutagen, a cover thumbnail is generated, and the result is written to SQLite. The scanner runs once at startup and then watches for changes via `watchdog`.

### Transcoding performance

Transcoding uses **ffmpeg** and runs per-request (no cache). A single stream is easily handled by any device:

| Source → Target | Pi 4 (1 core) | x86 (1 core) |
|---|---|---|
| FLAC → 192k Opus | ~3.5× realtime | ~15× realtime |
| FLAC → 128k MP3 | ~4× realtime | ~20× realtime |
| MP3 → 64k Opus | ~6× realtime | ~30× realtime |

> **Real-world:** A Pi 4 can serve 3–4 concurrent transcoded streams without breaking a sweat. Direct streaming (passthrough, no transcode) is essentially zero-CPU and limited only by your network bandwidth.

### Disk footprint

| Item | Size |
|---|---|
| App + dependencies | ~50 MB |
| Cover art cache | ~30–100 KB per album cover |
| SQLite DB (metadata only) | ~1 MB per 10,000 tracks |
| SQLite DB (with play history) | ~2–3 MB per 100,000 plays |

### Network

| Stream type | Bitrate per listener |
|---|---|
| Passthrough (original file) | Variable (320 kbps MP3 → 1,400 kbps FLAC) |
| High (192k Opus) | ~192 kbps |
| Medium (128k MP3) | ~128 kbps |
| Low (96k Opus) | ~96 kbps |
| Voice (64k Opus) | ~64 kbps |

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

<details>
<summary><b>Click to expand</b></summary>
<br>

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
├── .github/
│   └── workflows/
│       └── test.yml              # CI/CD pipeline (push/PR)
├── contrib/
│   └── discord-presence.py   # Discord RPC companion script
├── routes/
│   ├── __init__.py
│   ├── deps.py               # Shared utilities, auth helpers, Pydantic models
│   ├── auth.py               # Register, login, me, setup wizard
│   ├── tracks.py             # Tracks, albums, artists, genres, home
│   ├── streaming.py          # Transcode, stream, cover art, download album
│   ├── playlists.py          # Playlists CRUD, share, folders, favorites, export
│   ├── admin.py              # Dashboard, stats, users, rescan, LRC upload, lyrics batch scan
│   ├── lastfm.py             # Last.fm scrobbling (now playing, scrobble, connect)
│   └── subsonic.py           # Subsonic API compatibility layer (23 endpoints)
├── tests/
│   ├── __init__.py
│   └── test_api.py           # 30+ API tests (pytest + httpx)
├── Dockerfile                # Container image
├── docker-compose.yml        # Docker orchestration
├── server.py                 # FastAPI app (~80 lines, includes all route modules)
├── scanner.py                # File scanner + metadata extractor + companion .lrc reader + async
├── database.py               # SQLite schema + migrations + indexes
├── config.py                 # Configuration loader
├── install.sh                # System installer script
├── uninstall.sh              # Cleanup script
├── update.sh                 # Git-pull updater
├── moidify.service           # Systemd unit file
├── requirements.txt          # Python dependencies
├── static/
│   ├── index.html            # Main frontend
│   ├── setup.html            # First-run setup wizard
│   ├── shared.html           # Public shared playlist page
│   ├── shared-album.html     # Public shared album page
│   ├── admin.html            # Admin dashboard
│   ├── admin.css             # Admin dashboard styles
│   ├── admin.js              # Admin dashboard logic
│   ├── style.css             # @imports all CSS files
│   ├── logo.png              # App logo
│   ├── lang/                 # JSON translation files
│   │   ├── en.json
│   │   └── de.json
│   ├── css/                  # Split CSS by feature
│   │   ├── variables.css     # CSS vars, light mode overrides
│   │   ├── layout.css        # App grid, html/body
│   │   ├── sidebar.css       # Nav, playlists, pinned
│   │   ├── main-content.css  # Album/artist grids, track rows
│   │   ├── player.css        # Player bar, seek, volume
│   │   ├── queue.css         # Queue panel
│   │   ├── modal.css         # Overlay + modal
│   │   ├── settings.css      # Settings layout, toggles, EQ
│   │   ├── context-menu.css  # Right-click menu
│   │   ├── nowplaying.css    # Now-playing overlay
│   │   ├── overlays.css      # Fullscreen art, EQ panel, etc
│   │   ├── animations.css    # All keyframes
│   │   ├── liquid-glass.css  # Liquid Glass theme (frosted glass effects)
│   │   ├── features.css      # Misc feature styles
│   │   ├── mini-player.css   # Mini player
│   │   ├── toast.css         # Toast notifications
│   │   └── responsive.css    # All @media queries
│   └── js/
│       ├── state.js          # App state + utility functions
│       ├── icons.js          # SVG icon library
│       ├── api.js            # API client + auth + favorites + playlists
│       ├── i18n.js           # Internationalization (JSON translation loader)
│       ├── player.js         # Audio engine + EQ + transcoding
│       ├── queue.js          # Queue management + shuffle
│       ├── lyrics.js         # Lyrics fetching + synced display
│       ├── animations.js     # Visual effects (vinyl spin, CD hole, glow)
│       ├── app.js            # Event binding + session persistence + init
│       ├── ui/               # Split UI helpers
│       │   ├── toast.js
│       │   ├── modal.js
│       │   ├── settings.js
│       │   ├── context-menu.js
│       │   ├── eq-panel.js
│       │   ├── sleep-timer.js
│       │   └── search.js
│       └── views/            # Split page renderers
│           ├── home.js
│           ├── albums.js
│           ├── artists.js
│           ├── tracks.js
│           ├── playlists.js
│           ├── genres.js
│           ├── search.js
│           └── navigate.js
├── version.txt               # Current version
└── music/                    # Your music goes here (local dev)
```

</details>

---

## 📸 Screenshots

> <details>
> <summary><b>🖥️ Desktop (click to expand)</b></summary>
> <br>
> 
> <div align="center">
>   <img src="screenshots/desktop/screenshot-desktop-homepage.png" width="700" alt="Homepage">
>   <br>
>   <em>Home feed</em>
>   <br><br>
>   <img src="screenshots/desktop/screenshot-desktop-albums.png" width="700" alt="Albums">
>   <br>
>   <em>Album browser</em>
>   <br><br>
>   <img src="screenshots/desktop/screenshot-desktop-artists.png" width="700" alt="Artists">
>   <br>
>   <em>Artist grid</em>
>   <br><br>
>   <img src="screenshots/desktop/screenshot-desktop-all_tracks.png" width="700" alt="All Tracks">
>   <br>
>   <em>Track list with sortable columns</em>
>   <br><br>
>   <img src="screenshots/desktop/screenshot-desktop-genres.png" width="700" alt="Genres">
>   <br>
>   <em>Genre browsing</em>
>   <br><br>
>   <img src="screenshots/desktop/screenshot-desktop-lyrics.png" width="700" alt="Lyrics">
>   <br>
>   <em>Synced lyrics overlay</em>
>   <br><br>
>   <img src="screenshots/desktop/screenshot-desktop-settings.png" width="700" alt="Settings">
>   <br>
>   <em>Settings modal with equalizer</em>
> </div>
> </details>
> 
> <details>
> <summary><b>📱 Mobile (click to expand)</b></summary>
> <br>
> 
> <div align="center">
>   <img src="screenshots/mobile/screenshot-mobile-homepage.jpg" width="320" alt="Homepage">
>   <br>
>   <em>Home feed</em>
>   <br><br>
>   <img src="screenshots/mobile/screenshot-mobile-albums.jpg" width="320" alt="Albums">
>   <br>
>   <em>Album browser</em>
>   <br><br>
>   <img src="screenshots/mobile/screenshot-mobile-artists.jpg" width="320" alt="Artists">
>   <br>
>   <em>Artist grid</em>
>   <br><br>
>   <img src="screenshots/mobile/screenshot-mobile-tracks.jpg" width="320" alt="Tracks">
>   <br>
>   <em>Track list</em>
>   <br><br>
>   <img src="screenshots/mobile/screenshot-mobile-genres.jpg" width="320" alt="Genres">
>   <br>
>   <em>Genre browsing</em>
>   <br><br>
>   <img src="screenshots/mobile/screenshot-mobile-lyrics.jpg" width="320" alt="Lyrics">
>   <br>
>   <em>Lyrics view</em>
>   <br><br>
>   <img src="screenshots/mobile/screenshot-mobile-media_player.jpg" width="320" alt="Now Playing">
>   <br>
>   <em>Now playing</em>
> </div>
> </details>

---

## 🧩 Tech Stack

| Layer | Technology |
|---|---|
| **Backend** | Python + [FastAPI](https://fastapi.tiangolo.com/) + uvicorn |
| **Frontend** | Vanilla JavaScript (no framework, no build step) |
| **Database** | SQLite (via `sqlite3`) with WAL mode |
| **Metadata** | [Mutagen](https://mutagen.readthedocs.io/) |
| **File watching** | [watchdog](https://github.com/gorakhargosh/watchdog) |
| **Transcoding** | [ffmpeg](https://ffmpeg.org/) — Opus, MP3, FLAC, WAV |
| **Subsonic API** | Built-in `/rest/*` endpoints for third-party clients |

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
