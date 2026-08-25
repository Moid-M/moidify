#!/usr/bin/env bash
set -euo pipefail

APP_NAME="Moidify"
APP_DIR="/opt/moidify"
DATA_DIR="/var/lib/moidify"
CONFIG_DIR="/etc/moidify"
SERVICE_USER="moidify"
SERVICE_FILE="/etc/systemd/system/moidify.service"
PYTHON="python3"
PORT=8000
MAX_UPLOAD_SIZE_GB="2.5"
REPO_URL="https://github.com/Moid-M/moidify"
GIT_REPO_URL="${REPO_URL}.git"

# ─── Parse flags ─────────────────────────────────────────────────────────────
VERBOSE=false
CLI_MUSIC_DIR=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    -v|--verbose) VERBOSE=true; shift ;;
    --music-dir) CLI_MUSIC_DIR="$2"; shift 2 ;;
    --music-dir=*) CLI_MUSIC_DIR="${1#*=}"; shift ;;
    *) shift ;;
  esac
done

# ─── Visual helpers ───────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'
GREY='\033[90m'; BOLD='\033[1m'; DIM='\033[2m'; NC='\033[0m'

TOTAL_STEPS=14
CUR_STEP=0

step() {
  CUR_STEP=$((CUR_STEP + 1))
  echo ""
  echo -e "  ${BOLD}${CYAN}▶ [${CUR_STEP}/${TOTAL_STEPS}]${NC} ${BOLD}$1${NC}"
}

info()  { echo -e "     ${CYAN}ℹ${NC} $1"; }
ok()    { echo -e "     ${GREEN}✓${NC} $1"; }
warn()  { echo -e "     ${YELLOW}⚠${NC} $1"; }
err()   { echo -e "     ${RED}✗${NC} $1"; }

_spinner() {
  local pid=$1 frames="⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏" i=0
  while kill -0 "$pid" 2>/dev/null; do
    printf "\r\033[K     ${GREY}%s${NC}" "${frames:i++%10:1}"
    sleep 0.1
  done
  printf "\r\033[K"
}

# run_cmd "label" cmd... — shows a spinner (non-verbose) or live output (verbose)
run_cmd() {
  local label="$1"; shift
  if $VERBOSE; then
    echo -e "     ${CYAN}↻${NC} $label"
    "$@"
    return $?
  fi
  printf "     ${GREY}…${NC} %s" "$label"
  "$@" >/dev/null 2>&1 &
  local pid=$!
  _spinner "$pid"
  wait "$pid"
  local rc=$?
  if [ "$rc" -eq 0 ]; then echo -e "${GREEN} ✓${NC} $label"
  else echo -e "${RED} ✗${NC} $label"; fi
  return $rc
}

cleanup() {
  if [[ -n "${TMPDIR:-}" && "${TMPDIR:-}" != "$SCRIPT_DIR" ]]; then
    rm -rf "$TMPDIR"
  fi
}
trap cleanup EXIT

# ─── Banner ────────────────────────────────────────────────────────────────────
echo ""
echo -e "${CYAN}  ╔════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}  ║${NC}             ${BOLD}🎵  MOIDIFY INSTALLER${NC}             ${CYAN}║${NC}"
echo -e "${CYAN}  ╚════════════════════════════════════════════════╝${NC}"
echo -e "     ${DIM}Your music. Anywhere. No strings attached.${NC}"

# ─── Step 1: preconditions ─────────────────────────────────────────────────────
step "Checking preconditions"

if [[ $EUID -ne 0 ]]; then
  err "This installer must be run as root (sudo)."
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# ─── Detect distro ─────────────────────────────────────────────────────────────
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
ok "Running as root; package manager: ${PKG_MANAGER:-none detected}"

# ─── Step 2: system dependencies ───────────────────────────────────────────────
step "Installing system dependencies"
case "$PKG_MANAGER" in
  apt) PKG_LIST="python3 python3-pip python3-venv sqlite3 rsync git curl" ;;
  dnf) PKG_LIST="python3 python3-pip python3-virtualenv sqlite rsync git curl" ;;
  pacman) PKG_LIST="python python-pip python-virtualenv sqlite rsync git curl" ;;
  zypper) PKG_LIST="python3 python3-pip python3-virtualenv sqlite3 rsync git curl" ;;
  apk) PKG_LIST="python3 py3-pip py3-virtualenv sqlite rsync git curl" ;;
esac

if [[ -n "$PKG_MANAGER" ]]; then
  if [[ "$PKG_MANAGER" == "apt" ]]; then
    run_cmd "Updating package lists" apt update -qq
  fi
  if run_cmd "Installing base packages" bash -c "$INSTALL_CMD $PKG_LIST"; then
    ok "System dependencies ready."
  else
    err "Failed to install system dependencies."
    exit 1
  fi
else
  warn "No supported package manager found; assuming dependencies are already present."
fi

# ─── Step 3: service user ──────────────────────────────────────────────────────
step "Creating service user"
if id "$SERVICE_USER" &>/dev/null; then
  info "User $SERVICE_USER already exists."
else
  useradd --system --no-create-home --shell /usr/sbin/nologin "$SERVICE_USER"
  ok "Created system user: $SERVICE_USER"
fi

# ─── Step 4: directories ───────────────────────────────────────────────────────
step "Preparing directories"
install -d -o "$SERVICE_USER" -g "$SERVICE_USER" "$APP_DIR" "$DATA_DIR" "$DATA_DIR/music" "$DATA_DIR/covers"
install -d -o root -g root "$CONFIG_DIR"
ok "Directories ready."

MUSIC_DIR_INPUT="${CLI_MUSIC_DIR:-${DATA_DIR}/music}"
if [[ ! -d "$MUSIC_DIR_INPUT" ]]; then
  install -d -o "$SERVICE_USER" -g "$SERVICE_USER" "$MUSIC_DIR_INPUT"
else
  chown "$SERVICE_USER":"$SERVICE_USER" "$MUSIC_DIR_INPUT"
fi

# ─── Step 5: application files ──────────────────────────────────────────────────
step "Fetching application files"
if [[ -f "$SCRIPT_DIR/server.py" ]]; then
  info "Using local source files from $SCRIPT_DIR"
  VERSION=$(cat "$SCRIPT_DIR/version.txt" 2>/dev/null || echo "?")
  if command -v rsync &>/dev/null; then
    run_cmd "Copying files (rsync)" rsync -a --delete \
      --exclude='install.sh' --exclude='__pycache__' --exclude='*.pyc' --exclude='.git' \
      --exclude='venv' --exclude='music' --exclude='data' --exclude='covers' \
      "$SCRIPT_DIR/" "$APP_DIR/"
  else
    run_cmd "Copying files (cp)" cp -r "$SCRIPT_DIR"/* "$APP_DIR/"
  fi
  chown -R "$SERVICE_USER":"$SERVICE_USER" "$APP_DIR"
  ok "Application files copied to $APP_DIR"
else
  info "Downloading application files…"
  TMPDIR=$(mktemp -d)

  if command -v git &>/dev/null; then
    if run_cmd "Cloning repository" git clone --depth 1 "$GIT_REPO_URL" "$TMPDIR/repo"; then
      rm -rf "$TMPDIR/repo/.git" 2>/dev/null || true
      cp -r "$TMPDIR/repo"/* "$TMPDIR/" 2>/dev/null || true
      rm -rf "$TMPDIR/repo" 2>/dev/null || true
    fi
  fi

  if [[ ! -f "$TMPDIR/server.py" ]]; then
    if command -v curl &>/dev/null; then
      run_cmd "Downloading release tarball" bash -c \
        "curl -sL '${REPO_URL}/archive/refs/heads/main.tar.gz' | tar -xz -C '$TMPDIR' --strip-components=1"
    elif command -v wget &>/dev/null; then
      run_cmd "Downloading release tarball" bash -c \
        "wget -qO- '${REPO_URL}/archive/refs/heads/main.tar.gz' | tar -xz -C '$TMPDIR' --strip-components=1"
    fi
  fi

  if [[ ! -f "$TMPDIR/server.py" ]]; then
    err "Failed to download Moidify source."
    exit 1
  fi

  VERSION=$(cat "$TMPDIR/version.txt" 2>/dev/null || echo "?")

  rm -rf "$TMPDIR/install.sh" "$TMPDIR/__pycache__" "$TMPDIR/music" \
         "$TMPDIR/data" "$TMPDIR/covers" "$TMPDIR/.git" 2>/dev/null || true
  find "$TMPDIR" -name '__pycache__' -type d -exec rm -rf {} + 2>/dev/null || true
  find "$TMPDIR" -name '*.pyc' -delete 2>/dev/null || true

  if command -v rsync &>/dev/null; then
    run_cmd "Copying files (rsync)" rsync -a --delete "$TMPDIR/" "$APP_DIR/"
  else
    run_cmd "Copying files (cp)" cp -r "$TMPDIR"/* "$APP_DIR/"
  fi
  chown -R "$SERVICE_USER":"$SERVICE_USER" "$APP_DIR"
  ok "Application files copied to $APP_DIR"
fi

# ─── Step 6: CLI ───────────────────────────────────────────────────────────────
step "Installing command-line interface"
if ! run_cmd "Installing CLI" install -m 755 "$APP_DIR/moidify" /usr/local/bin/moidify; then
  warn "Could not install CLI to /usr/local/bin/moidify"
  warn "Run manually: sudo install -m 755 $APP_DIR/moidify /usr/local/bin/moidify"
fi

# ─── Step 7: virtual environment ───────────────────────────────────────────────
step "Creating Python virtual environment"
if ! run_cmd "Creating virtual environment" "$PYTHON" -m venv "$APP_DIR/venv"; then
  err "Failed to create virtual environment. Ensure python3-venv is installed."
  exit 1
fi
chown -R "$SERVICE_USER":"$SERVICE_USER" "$APP_DIR/venv"

# ─── Step 8: Python dependencies ───────────────────────────────────────────────
step "Installing Python dependencies"
if run_cmd "Installing Python dependencies" "$APP_DIR/venv/bin/pip" install --no-cache-dir -r "$APP_DIR/requirements.txt"; then
  ok "Python dependencies installed."
else
  err "Dependency install failed — this is usually a network issue."
  err "Re-run the installer; it is safe to run again."
  exit 1
fi

# ─── Step 9: yt-dlp ────────────────────────────────────────────────────────────
step "Installing yt-dlp (URL imports)"
mkdir -p "$APP_DIR/extra-pkgs"
if ! run_cmd "Installing yt-dlp" "$PYTHON" -m pip install --target="$APP_DIR/extra-pkgs" --upgrade --no-cache-dir yt-dlp; then
  warn "yt-dlp install failed — URL imports will be unavailable."
  warn "Install later with: pip install yt-dlp"
fi
chown -R "$SERVICE_USER":"$SERVICE_USER" "$APP_DIR/extra-pkgs" 2>/dev/null || true

# ─── Step 10: ffmpeg ───────────────────────────────────────────────────────────
step "Installing ffmpeg (transcoding)"
if ! command -v ffmpeg &>/dev/null && [[ -n "$PKG_MANAGER" ]]; then
  case "$PKG_MANAGER" in
    apt)    run_cmd "Installing ffmpeg" bash -c "DEBIAN_FRONTEND=noninteractive apt install -y ffmpeg" || warn "ffmpeg install skipped (install manually: sudo apt install ffmpeg)" ;;
    dnf)    run_cmd "Installing ffmpeg" bash -c "dnf install -y ffmpeg" || warn "ffmpeg install skipped" ;;
    pacman) run_cmd "Installing ffmpeg" bash -c "pacman -S --noconfirm ffmpeg" || warn "ffmpeg install skipped" ;;
    zypper) run_cmd "Installing ffmpeg" bash -c "zypper install -y ffmpeg" || warn "ffmpeg install skipped" ;;
    apk)    run_cmd "Installing ffmpeg" bash -c "apk add ffmpeg" || warn "ffmpeg install skipped" ;;
  esac
else
  info "ffmpeg already present or no package manager — skipping."
fi

# ─── Step 11: configuration ────────────────────────────────────────────────────
step "Writing configuration"
MAX_UPLOAD_SIZE_BYTES=$(python3 -c "print(int(float('$MAX_UPLOAD_SIZE_GB') * 1024 * 1024 * 1024))")
cat > "$CONFIG_DIR/config.json" <<CONF
{
  "music_dir": "$MUSIC_DIR_INPUT",
  "covers_dir": "$DATA_DIR/covers",
  "db_path": "$DATA_DIR/music.db",
  "port": $PORT,
  "max_upload_size": $MAX_UPLOAD_SIZE_BYTES
}
CONF
ok "Configuration written to $CONFIG_DIR/config.json"

# ─── Step 12: database ─────────────────────────────────────────────────────────
step "Initializing database"
if "$APP_DIR/venv/bin/python" -c "
import sys; sys.path.insert(0, '$APP_DIR')
from database import init_db; init_db()
" 2>&1; then
  ok "Database initialized."
else
  warn "Database init had issues (may be fine on first start)"
fi
chown "$SERVICE_USER":"$SERVICE_USER" "$DATA_DIR/music.db" 2>/dev/null || true

# ─── Step 13: systemd ──────────────────────────────────────────────────────────
step "Installing systemd service"
sed "s/--port 8000/--port $PORT/" "$APP_DIR/moidify.service" > "$SERVICE_FILE"
systemctl daemon-reload
ok "Systemd service installed on port $PORT."

# ─── Step 14: start ────────────────────────────────────────────────────────────
step "Starting Moidify"
systemctl enable moidify.service
systemctl restart moidify.service
sleep 2

if systemctl is-active --quiet moidify.service; then
  ok "${APP_NAME} is running!"
else
  warn "${APP_NAME} failed to start. Check: journalctl -u moidify.service -n 30 --no-pager"
fi

# ─── Summary ───────────────────────────────────────────────────────────────────
IP=$(ip route get 1 2>/dev/null | awk '{print $7}' || hostname -I 2>/dev/null | awk '{print $1}' || echo "localhost")
echo ""
echo -e "${GREEN}  ╔════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}  ║${NC}            ${BOLD}✓  Installation complete${NC}            ${GREEN}║${NC}"
echo -e "${GREEN}  ╚════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "     ${BOLD}Moidify v${VERSION}${NC} is running at:"
echo -e "        ${CYAN}http://${IP}:${PORT}${NC}"
echo ""
echo -e "     ${DIM}Music folder:${NC}   $MUSIC_DIR_INPUT"
echo -e "     ${DIM}Config:${NC}         $CONFIG_DIR/config.json"
echo -e "     ${DIM}Data:${NC}           $DATA_DIR"
echo -e "     ${DIM}Logs:${NC}           journalctl -u moidify.service -f"
echo ""
echo -e "     ${BOLD}Next:${NC} open the setup wizard to create your admin account:"
echo -e "        ${CYAN}http://${IP}:${PORT}/setup${NC}"
echo ""
echo -e "     ${DIM}Commands:${NC} run ${CYAN}moidify help${NC} to list all available commands"
echo ""
