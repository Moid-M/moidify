#!/usr/bin/env bash
set -euo pipefail

APP_NAME="Moidify"
APP_DIR="/opt/moidify"
DATA_DIR="/var/lib/moidify"
CONFIG_DIR="/etc/moidify"
SERVICE_USER="moidify"
SERVICE_FILE="/etc/systemd/system/moidify.service"
PYTHON="python3"

# ─── Colors ─────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
info()  { echo -e "${CYAN}::${NC} $1"; }
ok()    { echo -e "${GREEN}ok${NC}  $1"; }
warn()  { echo -e "${YELLOW}!!${NC} $1"; }
err()   { echo -e "${RED}!!${NC} $1"; }

cleanup() { [[ -d "$TMPDIR" ]] && rm -rf "$TMPDIR"; }
trap cleanup EXIT

# ─── Root check ─────────────────────────────────────────────────────────────
if [[ $EUID -ne 0 ]]; then
  err "This installer must be run as root (sudo)."
  exit 1
fi

echo ""
echo -e "${CYAN}╔══════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║       ${APP_NAME} Installer v1.1          ║${NC}"
echo -e "${CYAN}╚══════════════════════════════════════════╝${NC}"
echo ""

# ─── Port prompt ─────────────────────────────────────────────────────────────
read -r -p "Server port [8000]: " PORT
PORT="${PORT:-8000}"
ok "Port: $PORT"

# ─── Detect distro ──────────────────────────────────────────────────────────
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

info "Detected package manager: ${PKG_MANAGER:-none}"

# ─── System deps ────────────────────────────────────────────────────────────
info "Installing system dependencies..."
if [[ -n "$PKG_MANAGER" ]]; then
  case "$PKG_MANAGER" in
    apt) apt update -qq; $INSTALL_CMD python3 python3-pip python3-venv sqlite3 rsync ;;
    dnf) $INSTALL_CMD python3 python3-pip python3-virtualenv sqlite rsync ;;
    pacman) $INSTALL_CMD python python-pip python-virtualenv sqlite rsync ;;
    zypper) $INSTALL_CMD python3 python3-pip python3-virtualenv sqlite3 rsync ;;
    apk) $INSTALL_CMD python3 py3-pip py3-virtualenv sqlite rsync ;;
  esac
else
  warn "Unknown package manager. Ensure python3, pip, sqlite3, and rsync are installed."
fi
ok "System dependencies ready."

# ─── Service user ───────────────────────────────────────────────────────────
if id "$SERVICE_USER" &>/dev/null; then
  info "User $SERVICE_USER already exists."
else
  useradd --system --no-create-home --shell /usr/sbin/nologin "$SERVICE_USER"
  ok "Created system user: $SERVICE_USER"
fi

# ─── Directories ────────────────────────────────────────────────────────────
install -d -o "$SERVICE_USER" -g "$SERVICE_USER" "$APP_DIR" "$DATA_DIR" "$DATA_DIR/music" "$DATA_DIR/covers"
install -d -o root -g root "$CONFIG_DIR"
ok "Directories created."

# ─── Copy files ─────────────────────────────────────────────────────────────
info "Copying application files..."
TMPDIR=$(mktemp -d)

REPO_URL="https://github.com/Moid-M/moidify.git"
# Check if we're running from a local repo (has server.py) or from a pipe
SRC_DIR="$(cd "$(dirname "$0")" && pwd 2>/dev/null)"
if [[ -f "$SRC_DIR/server.py" ]]; then
  # Local install — copy from source
  cp -r "$SRC_DIR"/* "$TMPDIR/" 2>/dev/null || true
  cp -r "$SRC_DIR"/.[!.]* "$TMPDIR/" 2>/dev/null || true
else
  # Remote install (curl pipe) — clone the repo
  info "Downloading from $REPO_URL"
  if command -v git &>/dev/null; then
    git clone --depth 1 "$REPO_URL" "$TMPDIR/repo" 2>/dev/null || {
      rm -rf "$TMPDIR" 2>/dev/null || true
      TMPDIR=$(mktemp -d)
      # fallback: download archive
      info "Git not available, using archive..."
      ARCHIVE_URL="https://github.com/Moid-M/moidify/archive/refs/heads/main.tar.gz"
      if command -v curl &>/dev/null; then
        curl -sL "$ARCHIVE_URL" | tar -xz -C "$TMPDIR" --strip-components=1
      elif command -v wget &>/dev/null; then
        wget -qO- "$ARCHIVE_URL" | tar -xz -C "$TMPDIR" --strip-components=1
      else
        err "Cannot download source — install curl or git first."
        exit 1
      fi
    }
    if [[ -d "$TMPDIR/repo" ]]; then
      rm -rf "$TMPDIR/repo/.git" 2>/dev/null || true
      cp -r "$TMPDIR/repo"/* "$TMPDIR/" 2>/dev/null || true
      rm -rf "$TMPDIR/repo" 2>/dev/null || true
    fi
  else
    ARCHIVE_URL="https://github.com/Moid-M/moidify/archive/refs/heads/main.tar.gz"
    if command -v curl &>/dev/null; then
      curl -sL "$ARCHIVE_URL" | tar -xz -C "$TMPDIR" --strip-components=1
    elif command -v wget &>/dev/null; then
      wget -qO- "$ARCHIVE_URL" | tar -xz -C "$TMPDIR" --strip-components=1
    else
      err "Cannot download source — install curl or git first."
      exit 1
    fi
  fi
fi

# Clean up unnecessary files from the stage
rm -rf "$TMPDIR/install.sh" "$TMPDIR/uninstall.sh" "$TMPDIR/moidify.service" \
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

# ─── Virtual env ────────────────────────────────────────────────────────────
info "Setting up Python virtual environment..."
$PYTHON -m venv "$APP_DIR/venv"
"$APP_DIR/venv/bin/python" -m ensurepip --upgrade 2>/dev/null || true
chown -R "$SERVICE_USER":"$SERVICE_USER" "$APP_DIR/venv"
ok "Virtual environment ready."

# ─── Python deps ────────────────────────────────────────────────────────────
info "Installing Python dependencies..."
"$APP_DIR/venv/bin/pip" install --quiet --no-cache-dir -r "$APP_DIR/requirements.txt"
ok "Python dependencies installed."

# ─── Music dir ──────────────────────────────────────────────────────────────
echo ""
read -r -p "Music folder path [${DATA_DIR}/music]: " MUSIC_DIR_INPUT
MUSIC_DIR_INPUT="${MUSIC_DIR_INPUT:-${DATA_DIR}/music}"
MUSIC_DIR_INPUT="${MUSIC_DIR_INPUT/#\~/$HOME}"

if [[ ! -d "$MUSIC_DIR_INPUT" ]]; then
  install -d -o "$SERVICE_USER" -g "$SERVICE_USER" "$MUSIC_DIR_INPUT"
else
  chown "$SERVICE_USER":"$SERVICE_USER" "$MUSIC_DIR_INPUT"
fi
ok "Music directory: $MUSIC_DIR_INPUT"

# ─── Admin account prompt ────────────────────────────────────────────────────
echo ""
info "Create an admin account (recommended for setup)."
read -r -p "Admin username [admin]: " ADMIN_USER
ADMIN_USER="${ADMIN_USER:-admin}"
read -r -s -p "Admin password (leave blank to skip): " ADMIN_PASS
echo ""
if [[ -z "$ADMIN_PASS" ]]; then
  warn "No password set — admin account will not be created. Use the setup wizard at http://<ip>:$PORT/setup"
  ADMIN_SALT=""
  ADMIN_HASH=""
else
  ADMIN_SALT="$(python3 -c "import secrets; print(secrets.token_hex(16))")"
  ADMIN_HASH="$(python3 -c "
import hashlib, sys
salt = '$ADMIN_SALT'
pwd = sys.stdin.readline().strip()
h = hashlib.pbkdf2_hmac('sha256', pwd.encode(), salt.encode(), 100000)
print(h.hex())
" <<< "$ADMIN_PASS")"
  ok "Admin user '$ADMIN_USER' will be created."
fi

# ─── Config ─────────────────────────────────────────────────────────────────
cat > "$CONFIG_DIR/config.json" <<CONF
{
  "music_dir": "$MUSIC_DIR_INPUT",
  "covers_dir": "$DATA_DIR/covers",
  "db_path": "$DATA_DIR/music.db",
  "port": $PORT
}
CONF
ok "Config written to $CONFIG_DIR/config.json"

# ─── Init DB ────────────────────────────────────────────────────────────────
info "Initializing database..."
"$APP_DIR/venv/bin/python" -c "
import sys; sys.path.insert(0, '$APP_DIR')
from database import init_db; init_db()
" 2>&1 || warn "Database init had issues (may be fine on first start)"
chown "$SERVICE_USER":"$SERVICE_USER" "$DATA_DIR/music.db" 2>/dev/null || true
ok "Database initialized."

# ─── Create admin user in DB ────────────────────────────────────────────────
if [[ -n "$ADMIN_SALT" && -n "$ADMIN_HASH" ]]; then
  info "Creating admin user '$ADMIN_USER'..."
  "$APP_DIR/venv/bin/python" << PYEOF 2>&1 || warn "Failed to create admin user (you can use setup wizard)"
import sys; sys.path.insert(0, '$APP_DIR')
from database import get_connection
conn = get_connection()
existing = conn.execute("SELECT id FROM users WHERE username = ?", ("$ADMIN_USER",)).fetchone()
if not existing:
    conn.execute("INSERT INTO users (username, password_hash, salt, is_admin) VALUES (?, ?, ?, 1)",
                 ("$ADMIN_USER", "$ADMIN_HASH", "$ADMIN_SALT"))
    conn.commit()
    print("Admin user created.")
else:
    print("Admin user already exists, skipping.")
conn.close()
PYEOF
  ok "Admin user '$ADMIN_USER' created."
fi

# ─── Systemd ────────────────────────────────────────────────────────────────
sed "s/--port 8000/--port $PORT/" "$APP_DIR/moidify.service" > "$SERVICE_FILE"
systemctl daemon-reload
ok "Systemd service installed on port $PORT."

# ─── Start ──────────────────────────────────────────────────────────────────
info "Starting ${APP_NAME}..."
systemctl enable moidify.service
systemctl restart moidify.service
sleep 2

if systemctl is-active --quiet moidify.service; then
  ok "${APP_NAME} is running!"
else
  warn "${APP_NAME} failed to start. Check: journalctl -u moidify.service -n 30 --no-pager"
fi

# ─── Summary ────────────────────────────────────────────────────────────────
IP=$(ip route get 1 2>/dev/null | awk '{print $7}' || hostname -I 2>/dev/null | awk '{print $1}' || echo "localhost")
echo ""
echo -e "${CYAN}╔══════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║          Installation Complete           ║${NC}"
echo -e "${CYAN}╚══════════════════════════════════════════╝${NC}"
echo ""
echo -e "  ${GREEN}${APP_NAME}${NC} is now running at:"
echo -e "  ${CYAN}http://${IP}:${PORT}${NC}"
echo ""
echo -e "  ${YELLOW}Music folder:${NC}  $MUSIC_DIR_INPUT"
echo -e "  ${YELLOW}Config:${NC}       $CONFIG_DIR/config.json"
echo -e "  ${YELLOW}Data:${NC}         $DATA_DIR"
echo -e "  ${YELLOW}Logs:${NC}         journalctl -u moidify.service -f"
echo ""
echo -e "  ${YELLOW}Commands:${NC}"
echo -e "    restart:  systemctl restart moidify"
echo -e "    stop:     systemctl stop moidify"
echo -e "    status:   systemctl status moidify"
echo -e "    uninstall: $APP_DIR/uninstall.sh"
echo ""
echo -e "  Drop music into your music folder and it will appear automatically."
echo -e "  Open the setup wizard at ${CYAN}http://${IP}:${PORT}/setup${NC}"
echo ""
