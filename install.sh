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
[[ -t 0 ]] && INTERACTIVE=1
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

# ─── Admin account prompt ────────────────────────────────────────────────────
if [[ $INTERACTIVE -eq 1 ]]; then
  read -r -p "  Admin username [admin]: " ADMIN_USER </dev/tty
fi
ADMIN_USER="${ADMIN_USER:-admin}"
ADMIN_PASS=""
if [[ $INTERACTIVE -eq 1 ]]; then
  while true; do
    read -r -s -p "  Admin password (blank = skip): " ADMIN_PASS </dev/tty
    echo ""
    if [[ -z "$ADMIN_PASS" ]]; then
      warn "No password set — use setup wizard at http://<ip>:$PORT/setup"
      break
    fi
    if [[ ${#ADMIN_PASS} -lt 8 ]]; then
      err "Password must be at least 8 characters"; continue
    fi
    if ! [[ "$ADMIN_PASS" =~ [a-z] ]]; then
      err "Password must contain a lowercase letter"; continue
    fi
    if ! [[ "$ADMIN_PASS" =~ [A-Z] ]]; then
      err "Password must contain an uppercase letter"; continue
    fi
    if ! [[ "$ADMIN_PASS" =~ [0-9] ]]; then
      err "Password must contain a digit"; continue
    fi
    read -r -s -p "  Confirm password: " ADMIN_PASS2 </dev/tty
    echo ""
    if [[ "$ADMIN_PASS" == "$ADMIN_PASS2" ]]; then
      break
    fi
    err "Passwords do not match, try again."
  done
fi
if [[ -z "$ADMIN_PASS" ]]; then
  warn "No password set — use setup wizard at http://<ip>:$PORT/setup"
  ADMIN_SALT=""; ADMIN_HASH=""
else
  ADMIN_SALT="$(python3 -c "import secrets; print(secrets.token_hex(16))")"
  ADMIN_HASH="$(python3 -c "
import hashlib, sys
salt = '$ADMIN_SALT'
pwd = sys.stdin.readline().strip()
h = hashlib.pbkdf2_hmac('sha256', pwd.encode(), salt.encode(), 100000)
print(h.hex())
" <<< "$ADMIN_PASS")"
fi

# ─── Source files ─────────────────────────────────────────────────────────────
if [[ -f "$SCRIPT_DIR/server.py" ]]; then
  info "Using local source files from $SCRIPT_DIR"
  VERSION=$(cat "$SCRIPT_DIR/version.txt" 2>/dev/null || echo "?")
  if command -v rsync &>/dev/null; then
    rsync -a --delete --exclude='install.sh' --exclude='uninstall.sh' \
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

  rm -rf "$TMPDIR/install.sh" "$TMPDIR/uninstall.sh" \
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
info "Installing yt-dlp for YouTube/SoundCloud imports..."
mkdir -p "$APP_DIR/extra-pkgs"
if "$PYTHON" -m pip install --target="$APP_DIR/extra-pkgs" --upgrade --no-cache-dir yt-dlp 2>/dev/null; then
  ok "yt-dlp installed."
else
  warn "yt-dlp installation failed (URL import won't work). Install manually:"
  warn "  sudo $PYTHON -m pip install --target=$APP_DIR/extra-pkgs yt-dlp"
fi
chown -R "$SERVICE_USER":"$SERVICE_USER" "$APP_DIR/extra-pkgs" 2>/dev/null || true

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

# ─── Create admin user ───────────────────────────────────────────────────────
if [[ -n "${ADMIN_SALT:-}" && -n "${ADMIN_HASH:-}" ]]; then
  info "Creating admin user '$ADMIN_USER'..."
  ADMIN_USER="$ADMIN_USER" ADMIN_HASH="$ADMIN_HASH" ADMIN_SALT="$ADMIN_SALT" "$APP_DIR/venv/bin/python" -c "
import os, sys; sys.path.insert(0, '$APP_DIR')
from database import get_connection
conn = get_connection()
u = os.environ['ADMIN_USER']
h = os.environ['ADMIN_HASH']
s = os.environ['ADMIN_SALT']
existing = conn.execute('SELECT id FROM users WHERE username = ?', (u,)).fetchone()
if not existing:
    conn.execute('INSERT INTO users (username, password_hash, salt, is_admin) VALUES (?, ?, ?, 1)', (u, h, s))
    conn.commit()
    print('Admin user created.')
else:
    print('Admin user already exists, skipping.')
conn.close()
" 2>&1 || warn "Failed to create admin user (you can use setup wizard)"
fi

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
