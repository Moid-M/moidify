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

TOTAL_STEPS=15
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

# quick GitHub reachability check (silent, non-fatal by caller)
_check_connectivity() {
  curl -fsS --max-time 8 -o /dev/null https://github.com 2>/dev/null \
    || curl -fsS --max-time 8 -o /dev/null https://api.github.com 2>/dev/null
}

# Read a [Y/n] answer from /dev/tty; falls back to the default when non-interactive.
_confirm() {
  local q="$1" default="${2:-Y}" input=""
  if [[ -t 0 ]] || [[ -c /dev/tty ]]; then
    { read -r -p "  $q [$default]: " input; } </dev/tty 2>/dev/null || input="$default"
  fi
  input="${input:-$default}"; input="${input,,}"
  [[ "$input" == "y" || "$input" == "yes" ]]
}

# Ask for an explicit single-letter choice; loops until valid. Returns 1 if no
# terminal is available, so the caller can apply a safe non-interactive default.
_ask_choice() {
  local q="$1" allowed="$2" input=""
  while true; do
    if [[ -t 0 ]] || [[ -c /dev/tty ]]; then
      { read -r -p "  ${BOLD}${YELLOW}➤${NC} $q ${DIM}(type one, then press Enter)${NC}: " input; } </dev/tty 2>/dev/null || input=""
    else
      input=""
    fi
    input="${input,,}"
    if [[ -z "$input" ]]; then return 1; fi
    if [[ "$allowed" == *"$input"* ]]; then CHOICE="$input"; return 0; fi
    warn "Please answer with one of: ${allowed}."
  done
}

cleanup() {
  if [[ -n "${TMPDIR:-}" && "${TMPDIR:-}" != "$SCRIPT_DIR" ]]; then
    rm -rf "$TMPDIR"
  fi
}
trap cleanup EXIT

# ─── Install log ───────────────────────────────────────────────────────────────
LOGFILE="/var/log/moidify-install.log"
if [[ -w "$(dirname "$LOGFILE")" ]]; then
  exec > >(tee -a "$LOGFILE") 2>&1
fi

# ─── Banner ────────────────────────────────────────────────────────────────────
echo ""
echo -e "${CYAN}  ╔════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}  ║${NC}             ${BOLD}🎵  MOIDIFY INSTALLER${NC}             ${CYAN}║${NC}"
echo -e "${CYAN}  ╚════════════════════════════════════════════════╝${NC}"
echo -e "     ${DIM}Your music. Anywhere. No strings attached.${NC}"

# ─── Trust preamble ────────────────────────────────────────────────────────────
echo -e "  ${BOLD}This installer will:${NC}"
echo -e "    ${DIM}•${NC} install system dependencies (python, ffmpeg, …)"
echo -e "    ${DIM}•${NC} download Moidify into ${BOLD}${APP_DIR}${NC}"
echo -e "    ${DIM}•${NC} create a '${SERVICE_USER}' service user + systemd service"
echo -e "  ${DIM}Open source — review the code at ${REPO_URL}${NC}"
echo ""
echo -e "  ${DIM}Install log: ${LOGFILE}${NC}"
echo ""

# ─── Step 1: preconditions ─────────────────────────────────────────────────────
step "Checking preconditions"

if [[ $EUID -ne 0 ]]; then
  err "This installer must be run as root (sudo)."
  exit 1
fi

# Idempotency guard — don't silently clobber an existing install
REINSTALL=false
KEEP_CONFIG=false
if [[ -d "$APP_DIR" && -f "$APP_DIR/server.py" ]]; then
  echo -e "  ${YELLOW}⚠${NC} An existing Moidify installation was found at ${BOLD}$APP_DIR${NC}."
  if _ask_choice "Reinstall over it or cancel? [R]einstall / [C]ancel" "rc"; then
    case "$CHOICE" in
      r) info "Reinstalling over the existing installation."; REINSTALL=true ;;
      c) info "Aborting as requested."; exit 0 ;;
    esac
  else
    info "Non-interactive run — defaulting to Cancel (no changes made)."
    exit 0
  fi
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

if ! _check_connectivity; then
  err "No internet connection detected."
  err "An internet connection is required to download dependencies and Moidify."
  exit 1
fi
ok "Internet connection: ok"

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
  TMPDIR=$(mktemp -d)

  if ! _check_connectivity; then
    err "Cannot reach GitHub — check your internet connection and try again."
    exit 1
  fi
  info "Source: ${BOLD}${REPO_URL}${NC}  (branch: ${BOLD}main${NC})"

  ARCHIVE="$TMPDIR/moidify.tar.gz"
  FETCHED=false

  # Preferred: shallow git clone (shows progress)
  if command -v git &>/dev/null; then
    printf "     ${GREY}↓${NC} Cloning repository (shallow)…\n"
    if git clone --depth 1 --progress "$GIT_REPO_URL" "$TMPDIR/repo" 2>&1 | cat; then
      if [[ -f "$TMPDIR/repo/server.py" ]]; then
        VERSION=$(cat "$TMPDIR/repo/version.txt" 2>/dev/null || echo "?")
        rm -rf "$TMPDIR/repo/.git"
        cp -r "$TMPDIR/repo"/* "$TMPDIR/" 2>/dev/null || true
        rm -rf "$TMPDIR/repo"
        FETCHED=true
        ok "Repository cloned (v$VERSION)"
      fi
    else
      warn "Git clone failed — falling back to release archive."
    fi
  fi

  # Fallback: download release tarball with a real progress bar
  if [[ "$FETCHED" != "true" ]]; then
    printf "     ${GREY}↓${NC} Downloading release archive…\n"
    if curl -fL# -o "$ARCHIVE" "${REPO_URL}/archive/refs/heads/main.tar.gz" 2>&1; then
      SIZE=$(du -h "$ARCHIVE" 2>/dev/null | cut -f1)
      printf "     ${GREEN}✓${NC} Downloaded archive (%s)\n" "${SIZE:-?}"
      if tar -xzf "$ARCHIVE" -C "$TMPDIR" --strip-components=1 2>/dev/null; then
        VERSION=$(cat "$TMPDIR/version.txt" 2>/dev/null || echo "?")
        FETCHED=true
      fi
    elif command -v wget &>/dev/null; then
      wget -q --show-progress -O "$ARCHIVE" "${REPO_URL}/archive/refs/heads/main.tar.gz" 2>&1
      if tar -xzf "$ARCHIVE" -C "$TMPDIR" --strip-components=1 2>/dev/null; then
        VERSION=$(cat "$TMPDIR/version.txt" 2>/dev/null || echo "?")
        FETCHED=true
      fi
    fi
  fi

  if [[ "$FETCHED" != "true" || ! -f "$TMPDIR/server.py" ]]; then
    err "Failed to download Moidify source."
    exit 1
  fi

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
  ok "Application files copied to $APP_DIR (v$VERSION)"
fi

# ─── Step 6: CLI ───────────────────────────────────────────────────────────────
step "Installing command-line interface"
if ! run_cmd "Installing CLI" install -m 755 "$APP_DIR/moidify" /usr/local/bin/moidify; then
  warn "Could not install CLI to /usr/local/bin/moidify"
  warn "Run manually: sudo install -m 755 $APP_DIR/moidify /usr/local/bin/moidify"
fi

# ─── Step 7: virtual environment ───────────────────────────────────────────────
step "Creating Python virtual environment"
if ! command -v "$PYTHON" &>/dev/null; then
  err "python3 not found after installing system dependencies."
  exit 1
fi
if ! "$PYTHON" -c "import sys; sys.exit(0 if sys.version_info>=(3,9) else 1)"; then
  err "Python 3.9 or newer is required to run Moidify."
  exit 1
fi
ok "Python $(python3 -c 'import sys;print("%d.%d"%sys.version_info[:2])') detected."
if ! run_cmd "Creating virtual environment" "$PYTHON" -m venv "$APP_DIR/venv"; then
  err "Failed to create virtual environment. Ensure python3-venv is installed."
  exit 1
fi
chown -R "$SERVICE_USER":"$SERVICE_USER" "$APP_DIR/venv"

# ─── Step 8: Python dependencies (resilient) ───────────────────────────────────
step "Installing Python dependencies"
install_ok=false
for attempt in 1 2 3; do
  if run_cmd "Installing Python dependencies (attempt $attempt/3)" \
        "$APP_DIR/venv/bin/pip" install --no-cache-dir --default-timeout=120 --retries=10 -r "$APP_DIR/requirements.txt"; then
    install_ok=true
    break
  fi
  warn "Dependency install failed (attempt $attempt/3) — retrying in 3s…"
  sleep 3
done
if $install_ok; then
  ok "Python dependencies installed."
else
  err "Dependency install failed — this is usually a network or DNS issue."
  err "A full log was written to ${LOGFILE}."
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

# On reinstall, ask whether to keep the existing config (port / music folder)
if $REINSTALL && [[ -f "$CONFIG_DIR/config.json" ]]; then
  if _ask_choice "Keep existing config (port/music folder) or overwrite with defaults? [K]eep / [O]verwrite" "ko"; then
    case "$CHOICE" in
      k) KEEP_CONFIG=true ;;
      o) KEEP_CONFIG=false ;;
    esac
  else
    KEEP_CONFIG=true   # non-interactive reinstall: keep by default
  fi
fi

if $KEEP_CONFIG && [[ -f "$CONFIG_DIR/config.json" ]]; then
  info "Keeping existing config at $CONFIG_DIR/config.json"
  CFG_PORT=$(python3 -c "import json; print(json.load(open('$CONFIG_DIR/config.json')).get('port', $PORT))" 2>/dev/null || echo "$PORT")
  PORT="$CFG_PORT"
  CFG_MUSIC=$(python3 -c "import json; print(json.load(open('$CONFIG_DIR/config.json')).get('music_dir', ''))" 2>/dev/null || true)
  if [[ -n "$CFG_MUSIC" ]]; then
    MUSIC_DIR_INPUT="$CFG_MUSIC"
    install -d -o "$SERVICE_USER" -g "$SERVICE_USER" "$MUSIC_DIR_INPUT" 2>/dev/null || true
  fi
else
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
fi

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

# ─── Step 14: firewall ─────────────────────────────────────────────────────────
step "Configuring firewall"
FW_TOOL=""
if command -v ufw &>/dev/null; then FW_TOOL="ufw";
elif command -v firewall-cmd &>/dev/null; then FW_TOOL="firewalld"; fi

if [[ -z "$FW_TOOL" ]]; then
  info "No supported firewall tool (ufw/firewalld) found — skipping."
  info "If this host is behind another firewall, open port $PORT manually."
elif _confirm "Open port $PORT in the firewall ($FW_TOOL)? [Y/n]"; then
  if [[ "$FW_TOOL" == "ufw" ]]; then
    if ! run_cmd "Opening port $PORT (ufw)" ufw allow "$PORT"/tcp; then
      warn "Could not open firewall port — open $PORT manually if needed."
    fi
  else
    if ! run_cmd "Opening port $PORT (firewalld)" bash -c "firewall-cmd --add-port=$PORT/tcp --permanent && firewall-cmd --reload"; then
      warn "Could not open firewall port — open $PORT manually if needed."
    fi
  fi
else
  info "Skipping firewall changes — open port $PORT manually if needed."
fi

# ─── Step 15: start ────────────────────────────────────────────────────────────
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
echo -e "     ${DIM}Install log:${NC}    $LOGFILE"
echo -e "     ${DIM}Logs:${NC}           journalctl -u moidify.service -f"
echo ""
echo -e "     ${BOLD}Next:${NC} open the setup wizard to create your admin account:"
echo -e "        ${CYAN}http://${IP}:${PORT}/setup${NC}"
echo ""
echo -e "     ${DIM}Commands:${NC} run ${CYAN}moidify help${NC} to list all available commands"
echo ""
