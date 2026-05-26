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

# Try git pull first (for git-based installs)
if [[ -d "$APP_DIR/.git" ]]; then
  cd "$APP_DIR"
  git stash --include-untracked 2>/dev/null || true
  git pull
else
  # Download latest archive
  TMPDIR=$(mktemp -d)
  curl -sL "${REPO_URL}/archive/refs/heads/main.tar.gz" | tar -xz -C "$TMPDIR" --strip-components=1
  rsync -a --delete "$TMPDIR/" "$APP_DIR/" 2>/dev/null || cp -r "$TMPDIR"/* "$APP_DIR/"
  rm -rf "$TMPDIR"
fi

# Install any new Python deps
"$APP_DIR/venv/bin/pip" install --quiet --no-cache-dir -r "$APP_DIR/requirements.txt"

# Apply correct ownership
chown -R "$SERVICE_USER":"$SERVICE_USER" "$APP_DIR"

# Restart service
systemctl restart moidify.service

echo ""
echo "Moidify updated and restarted."
echo "Check status: systemctl status moidify"
