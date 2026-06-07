#!/usr/bin/env bash
set -euo pipefail

APP_NAME="Moidify"
APP_DIR="/opt/moidify"
DATA_DIR="/var/lib/moidify"
CONFIG_DIR="/etc/moidify"
SERVICE_USER="moidify"
SERVICE_FILE="/etc/systemd/system/moidify.service"
PYTHON="python3"
REPO_URL="https://github.com/Moid-M/moidify"
GIT_REPO_URL="${REPO_URL}.git"

# ─── Parse flags ─────────────────────────────────────────────────────────────
VERBOSE=false
SHOW_VERSION=false
CLI_MUSIC_DIR=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    -v|--verbose) VERBOSE=true; shift ;;
    -V|--version) SHOW_VERSION=true; shift ;;
    --music-dir) CLI_MUSIC_DIR="$2"; shift 2 ;;
    --music-dir=*) CLI_MUSIC_DIR="${1#*=}"; shift ;;
    *) shift ;;
  esac
done

# Quick version display (no installation)
if $SHOW_VERSION; then
  # Try to get version from local copy first, then GitHub
  if [[ -f "$(dirname "$0")/version.txt" ]]; then
    cat "$(dirname "$0")/version.txt"
  else
    curl -sL "${REPO_URL}/raw/main/version.txt" 2>/dev/null || echo "unknown"
  fi
  exit 0
fi

# ─── Helpers ─────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
info()  { echo -e "${CYAN}::${NC} $1"; }
ok()    { echo -e "${GREEN}ok${NC}  $1"; }
warn()  { echo -e "${YELLOW}!!${NC} $1"; }
err()   { echo -e "${RED}!!${NC} $1"; }

if $VERBOSE; then
  LOG=""     # show everything
else
  LOG="--quiet"  # suppress subcommand output
fi

cleanup() {
  if [[ -n "${TMPDIR:-}" && "${TMPDIR:-}" != "$SCRIPT_DIR" ]]; then
    rm -rf "$TMPDIR"
  fi
}
trap cleanup EXIT

# ─── Root check ──────────────────────────────────────────────────────────────
if [[ $EUID -ne 0 ]]; then
  err "This installer must be run as root (sudo)."
  exit 1
fi

INTERACTIVE=0
if [[ -t 0 ]]; then
  INTERACTIVE=1
elif [ -c /dev/tty ] 2>/dev/null; then
  # stdin is piped but we can still prompt via /dev/tty
  INTERACTIVE=1
fi
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# ─── Header ──────────────────────────────────────────────────────────────────
echo ""
echo -e "${CYAN}  ╔═══════════════════════════╗${NC}"
echo -e "${CYAN}  ║     Moidify Installer     ║${NC}"
echo -e "${CYAN}  ╚═══════════════════════════╝${NC}"
echo ""

# ─── Detect distro ───────────────────────────────────────────────────────────
PKG_MANAGER=""
INSTALL_CMD=""
if command -v apt &>/dev/null; then
  PKG_MANAGER="apt"; INSTALL_CMD="apt install -y"
elif command -v dnf &>/dev/null; then
  PKG_MANAGER="dnf"; INSTALL_CMD="dnf install -y"
elif command -v pacman &>/dev/null; then
  PKG_MANAGER="pacman"; INSTALL_CMD="pacman -S --noconfirm"
elif command -v zypper &>/dev/null; then
  PKG_MANAGER="zypper"; INSTALL_CMD="zypper install -y"
elif command -v apk &>/dev/null; then
  PKG_MANAGER="apk"; INSTALL_CMD="apk add"
fi

# ─── System deps ─────────────────────────────────────────────────────────────
case "$PKG_MANAGER" in
  apt) PKG_LIST="python3 python3-pip python3-venv sqlite3 rsync git curl" ;;
  dnf) PKG_LIST="python3 python3-pip python3-virtualenv sqlite rsync git curl" ;;
  pacman) PKG_LIST="python python-pip python-virtualenv sqlite rsync git curl" ;;
  zypper) PKG_LIST="python3 python3-pip python3-virtualenv sqlite3 rsync git curl" ;;
  apk) PKG_LIST="python3 py3-pip py3-virtualenv sqlite rsync git curl" ;;
esac

if [[ -n "$PKG_MANAGER" ]]; then
  info "Installing system dependencies..."
  case "$PKG_MANAGER" in
    apt) apt update -qq $LOG ;;
  esac
  if $VERBOSE; then
    $INSTALL_CMD $PKG_LIST
  else
    $INSTALL_CMD $PKG_LIST >/dev/null 2>&1
  fi
  ok "System dependencies ready."
fi

# ─── Service user ────────────────────────────────────────────────────────────
if id "$SERVICE_USER" &>/dev/null; then
  info "User $SERVICE_USER already exists."
else
  useradd --system --no-create-home --shell /usr/sbin/nologin "$SERVICE_USER"
  ok "Created system user: $SERVICE_USER"
fi

# ─── Directories ─────────────────────────────────────────────────────────────
install -d -o "$SERVICE_USER" -g "$SERVICE_USER" "$APP_DIR" "$DATA_DIR" "$DATA_DIR/music" "$DATA_DIR/covers"
install -d -o root -g root "$CONFIG_DIR"

# ─── Port ────────────────────────────────────────────────────────────────────
if [[ $INTERACTIVE -eq 1 ]]; then
  read -r -p "  Server port [8000]: " PORT </dev/tty
fi
PORT="${PORT:-8000}"

# ─── Music dir ───────────────────────────────────────────────────────────────
if [[ -n "$CLI_MUSIC_DIR" ]]; then
  MUSIC_DIR_INPUT="$CLI_MUSIC_DIR"
  info "Using music dir from --music-dir: $MUSIC_DIR_INPUT"
elif [[ $INTERACTIVE -eq 1 ]]; then
  read -r -p "  Music folder path [${DATA_DIR}/music]: " MUSIC_DIR_INPUT </dev/tty
  MUSIC_DIR_INPUT="${MUSIC_DIR_INPUT:-${DATA_DIR}/music}"
else
  MUSIC_DIR_INPUT="${DATA_DIR}/music"
fi
MUSIC_DIR_INPUT="${MUSIC_DIR_INPUT/#\~/$HOME}"
if [[ ! -d "$MUSIC_DIR_INPUT" ]]; then
  install -d -o "$SERVICE_USER" -g "$SERVICE_USER" "$MUSIC_DIR_INPUT"
else
  chown "$SERVICE_USER":"$SERVICE_USER" "$MUSIC_DIR_INPUT"
fi

# ─── Upload size limit ────────────────────────────────────────────────────────
MAX_UPLOAD_SIZE_GB="2.5"
if [[ $INTERACTIVE -eq 1 ]]; then
  read -r -p "  Max upload size in GB [2.5]: " MAX_UPLOAD_SIZE_GB_INPUT </dev/tty
  MAX_UPLOAD_SIZE_GB="${MAX_UPLOAD_SIZE_GB_INPUT:-$MAX_UPLOAD_SIZE_GB}"
fi
# Convert to bytes
MAX_UPLOAD_SIZE_BYTES=$(python3 -c "print(int(float('$MAX_UPLOAD_SIZE_GB') * 1024 * 1024 * 1024))")

# ─── Admin account ────────────────────────────────────────────────────────────
info "Admin account will be created via the web wizard at http://<ip>:$PORT/setup"

# ─── Source files ─────────────────────────────────────────────────────────────
if [[ -f "$SCRIPT_DIR/server.py" ]]; then
  info "Using local source files from $SCRIPT_DIR"
  VERSION=$(cat "$SCRIPT_DIR/version.txt" 2>/dev/null || echo "?")
  if command -v rsync &>/dev/null; then
    rsync -a --delete --exclude='install.sh' \
      --exclude='__pycache__' --exclude='*.pyc' --exclude='.git' \
      --exclude='venv' --exclude='music' --exclude='data' --exclude='covers' \
      "$SCRIPT_DIR/" "$APP_DIR/"
  else
    cp -r "$SCRIPT_DIR"/* "$APP_DIR/"
  fi
  chown -R "$SERVICE_USER":"$SERVICE_USER" "$APP_DIR"
  ok "Application files copied to $APP_DIR"
else
  info "Downloading application files..."
  TMPDIR=$(mktemp -d)

  if command -v git &>/dev/null; then
    git clone --depth 1 "$GIT_REPO_URL" "$TMPDIR/repo" 2>&1 | tail -1 || true
    if [[ -d "$TMPDIR/repo" ]]; then
      rm -rf "$TMPDIR/repo/.git" 2>/dev/null || true
      cp -r "$TMPDIR/repo"/* "$TMPDIR/" 2>/dev/null || true
      rm -rf "$TMPDIR/repo" 2>/dev/null || true
    fi
  fi

  if [[ ! -f "$TMPDIR/server.py" ]]; then
    if command -v curl &>/dev/null; then
      curl -sL "${REPO_URL}/archive/refs/heads/main.tar.gz" | tar -xz -C "$TMPDIR" --strip-components=1
    elif command -v wget &>/dev/null; then
      wget -qO- "${REPO_URL}/archive/refs/heads/main.tar.gz" | tar -xz -C "$TMPDIR" --strip-components=1
    fi
  fi

  if [[ ! -f "$TMPDIR/server.py" ]]; then
    err "Failed to download Moidify source."
    exit 1
  fi

  VERSION=$(cat "$TMPDIR/version.txt" 2>/dev/null || echo "?")

  rm -rf "$TMPDIR/install.sh" \
         "$TMPDIR/__pycache__" "$TMPDIR/music" "$TMPDIR/data" "$TMPDIR/covers" \
         "$TMPDIR/.git" 2>/dev/null || true
  find "$TMPDIR" -name '__pycache__' -type d -exec rm -rf {} + 2>/dev/null || true
  find "$TMPDIR" -name '*.pyc' -delete 2>/dev/null || true

  if command -v rsync &>/dev/null; then
    rsync -a --delete "$TMPDIR/" "$APP_DIR/"
  else
    cp -r "$TMPDIR"/* "$APP_DIR/"
  fi
  chown -R "$SERVICE_USER":"$SERVICE_USER" "$APP_DIR"
  ok "Application files copied to $APP_DIR"
fi

# ─── CLI ──────────────────────────────────────────────────────────────────────
if install -m 755 "$APP_DIR/moidify" /usr/local/bin/moidify 2>/dev/null; then
  ok "CLI installed: /usr/local/bin/moidify"
else
  warn "Could not install CLI to /usr/local/bin/moidify"
  warn "Run manually: sudo install -m 755 $APP_DIR/moidify /usr/local/bin/moidify"
fi

# ─── Virtual env + deps ──────────────────────────────────────────────────────
info "Setting up Python virtual environment..."
$PYTHON -m venv "$APP_DIR/venv"
chown -R "$SERVICE_USER":"$SERVICE_USER" "$APP_DIR/venv"

info "Installing Python dependencies..."
if $VERBOSE; then
  "$APP_DIR/venv/bin/pip" install --no-cache-dir -r "$APP_DIR/requirements.txt"
else
  "$APP_DIR/venv/bin/pip" install --quiet --no-cache-dir -r "$APP_DIR/requirements.txt"
fi
ok "Python dependencies installed."

# ─── yt-dlp (for URL imports) ────────────────────────────────────────────────
info "Installing yt-dlp..."
mkdir -p "$APP_DIR/extra-pkgs"
if $VERBOSE; then
  "$PYTHON" -m pip install --target="$APP_DIR/extra-pkgs" --upgrade --no-cache-dir yt-dlp && ok "yt-dlp installed." || warn "yt-dlp install failed"
else
  "$PYTHON" -m pip install --target="$APP_DIR/extra-pkgs" --upgrade --no-cache-dir yt-dlp >/dev/null 2>&1 && ok "yt-dlp installed." || warn "yt-dlp install failed"
fi
chown -R "$SERVICE_USER":"$SERVICE_USER" "$APP_DIR/extra-pkgs" 2>/dev/null || true

# ─── ffmpeg (for audio transcoding) ──────────────────────────────────────────
if ! command -v ffmpeg &>/dev/null && [[ -n "$PKG_MANAGER" ]]; then
  info "Installing ffmpeg..."
  case "$PKG_MANAGER" in
    apt) FFMPEG_PKG="ffmpeg" ;;
    dnf) FFMPEG_PKG="ffmpeg" ;;
    pacman) FFMPEG_PKG="ffmpeg" ;;
    zypper) FFMPEG_PKG="ffmpeg" ;;
    apk) FFMPEG_PKG="ffmpeg" ;;
  esac
  if $VERBOSE; then
    $INSTALL_CMD $FFMPEG_PKG && ok "ffmpeg installed." || warn "ffmpeg install failed"
  else
    $INSTALL_CMD $FFMPEG_PKG >/dev/null 2>&1 && ok "ffmpeg installed." || warn "ffmpeg install failed"
  fi
fi

# ─── Config ──────────────────────────────────────────────────────────────────
cat > "$CONFIG_DIR/config.json" <<CONF
{
  "music_dir": "$MUSIC_DIR_INPUT",
  "covers_dir": "$DATA_DIR/covers",
  "db_path": "$DATA_DIR/music.db",
  "port": $PORT,
  "max_upload_size": $MAX_UPLOAD_SIZE_BYTES
}
CONF

# ─── Init DB ─────────────────────────────────────────────────────────────────
info "Initializing database..."
"$APP_DIR/venv/bin/python" -c "
import sys; sys.path.insert(0, '$APP_DIR')
from database import init_db; init_db()
" 2>&1 || warn "Database init had issues (may be fine on first start)"
chown "$SERVICE_USER":"$SERVICE_USER" "$DATA_DIR/music.db" 2>/dev/null || true

# ─── Systemd ─────────────────────────────────────────────────────────────────
sed "s/--port 8000/--port $PORT/" "$APP_DIR/moidify.service" > "$SERVICE_FILE"
systemctl daemon-reload
ok "Systemd service installed on port $PORT."

# ─── Start ───────────────────────────────────────────────────────────────────
info "Starting ${APP_NAME}..."
systemctl enable moidify.service
systemctl restart moidify.service
sleep 2

if systemctl is-active --quiet moidify.service; then
  ok "${APP_NAME} is running!"
else
  warn "${APP_NAME} failed to start. Check: journalctl -u moidify.service -n 30 --no-pager"
fi

# ─── Summary ─────────────────────────────────────────────────────────────────
IP=$(ip route get 1 2>/dev/null | awk '{print $7}' || hostname -I 2>/dev/null | awk '{print $1}' || echo "localhost")
echo ""
echo -e "${CYAN}  ╔═══════════════════════════╗${NC}"
echo -e "${CYAN}  ║   Installation Complete   ║${NC}"
echo -e "${CYAN}  ╚═══════════════════════════╝${NC}"
echo ""
echo -e "  ${GREEN}${APP_NAME} v${VERSION}${NC} running at:"
echo -e "  ${CYAN}http://${IP}:${PORT}${NC}"
echo ""
echo -e "  ${YELLOW}Music folder:${NC}  $MUSIC_DIR_INPUT"
echo -e "  ${YELLOW}Upload limit:${NC}  ${MAX_UPLOAD_SIZE_GB} GB"
echo -e "  ${YELLOW}Config:${NC}       $CONFIG_DIR/config.json"
echo -e "  ${YELLOW}Data:${NC}         $DATA_DIR"
echo -e "  ${YELLOW}Logs:${NC}         journalctl -u moidify.service -f"
echo ""
echo -e "  ${YELLOW}Commands:${NC}"
echo -e "    help:     moidify -h"
echo -e "    start:    moidify start"
echo -e "    stop:     moidify stop"
echo -e "    restart:  moidify restart"
echo -e "    status:   moidify status"
echo -e "    logs:     moidify logs"
echo -e "    config:   moidify config"
echo -e "    update:   moidify update"
echo -e "    reset-password: moidify reset-password"
echo -e "    download: moidify download <url>"
echo -e "    uninstall: ${APP_DIR}/uninstall.sh"
echo ""
echo -e "  Drop music into your folder — files appear automatically."
echo -e "  Open setup wizard: ${CYAN}http://${IP}:${PORT}/setup${NC}"
echo ""

if ! $VERBOSE; then
  echo -e "  ${YELLOW}Tip:${NC} Run with ${CYAN}-v${NC} for verbose output."
fi
echo ""
