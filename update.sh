#!/usr/bin/env bash
set -euo pipefail

APP_DIR="/opt/moidify"
SERVICE_USER="moidify"
REPO_URL="https://github.com/Moid-M/moidify.git"

if [[ $EUID -ne 0 ]]; then
  echo "This must be run as root (sudo)."
  exit 1
fi

echo "Updating Moidify..."

# Use local sources if running from a dev copy (not the installed dir)
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
if [[ -f "$SCRIPT_DIR/server.py" && "$SCRIPT_DIR" != "$APP_DIR" ]]; then
  echo "Using local source files from $SCRIPT_DIR"
  rsync -a --delete --exclude=/venv "$SCRIPT_DIR/" "$APP_DIR/"
# Try git pull (for git-based installs)
elif [[ -d "$APP_DIR/.git" ]]; then
  cd "$APP_DIR"
  git stash --include-untracked 2>/dev/null || true
  git pull
else
  # Download latest archive
  TMPDIR=$(mktemp -d)
  curl -sL "${REPO_URL}/archive/refs/heads/main.tar.gz" | tar -xz -C "$TMPDIR" --strip-components=1
  rsync -a --delete "$TMPDIR/" "$APP_DIR/"
  rm -rf "$TMPDIR"
fi

# Recreate venv if missing
if [[ ! -f "$APP_DIR/venv/bin/pip" ]]; then
  echo "Setting up virtual environment..."
  python3 -m venv "$APP_DIR/venv"
fi

# Install any new Python deps
"$APP_DIR/venv/bin/pip" install --quiet --no-cache-dir -r "$APP_DIR/requirements.txt"

# Restart service
systemctl restart moidify.service

echo ""
echo "Moidify updated and restarted."
echo "Check status: systemctl status moidify"
