#!/usr/bin/env bash
set -euo pipefail

APP_DIR="/opt/moidify"
SERVICE_USER="moidify"

if [[ $EUID -ne 0 ]]; then
  echo "This must be run as root (sudo)."
  exit 1
fi

if [[ ! -d "$APP_DIR/.git" ]]; then
  echo "Moidify was not installed via git. Re-run the installer or clone the repo manually."
  exit 1
fi

echo "Updating Moidify..."
cd "$APP_DIR"

# Stash any local changes (e.g. config tweaks)
git stash --include-untracked 2>/dev/null || true

# Pull latest
git pull

# Install any new Python deps
"$APP_DIR/venv/bin/pip" install --quiet --no-cache-dir -r "$APP_DIR/requirements.txt"

# Apply correct ownership
chown -R "$SERVICE_USER":"$SERVICE_USER" "$APP_DIR"

# Restart service
systemctl restart moidify.service

echo ""
echo "Moidify updated and restarted."
echo "Check status: systemctl status moidify"
