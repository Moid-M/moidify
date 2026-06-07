#!/usr/bin/env bash
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
info()  { echo -e "${CYAN}::${NC} $1"; }
ok()    { echo -e "${GREEN}ok${NC}  $1"; }
warn()  { echo -e "${YELLOW}!!${NC} $1"; }

if [[ $EUID -ne 0 ]]; then
  echo -e "${RED}!!${NC} This must be run as root (sudo)."
  exit 1
fi

echo ""
echo -e "${YELLOW}╔══════════════════════════════════════════╗${NC}"
echo -e "${YELLOW}║       Moidify Uninstaller                ║${NC}"
echo -e "${YELLOW}╚══════════════════════════════════════════╝${NC}"
echo ""

read -r -p "This will remove Moidify and ALL data. Continue? [y/N] " CONFIRM
if [[ ! "$CONFIRM" =~ ^[Yy]$ ]]; then
  info "Cancelled."
  exit 0
fi

# Stop and disable service
if systemctl is-active --quiet moidify.service 2>/dev/null; then
  systemctl stop moidify.service
  ok "Service stopped."
fi
systemctl disable moidify.service 2>/dev/null || true
rm -f /etc/systemd/system/moidify.service
systemctl daemon-reload
ok "Service removed."

# Remove app files
if [[ -d /opt/moidify ]]; then
  rm -rf /opt/moidify
  ok "Removed /opt/moidify"
fi

# Remove config
if [[ -d /etc/moidify ]]; then
  rm -rf /etc/moidify
  ok "Removed /etc/moidify"
fi

# Remove system user
if id moidify &>/dev/null; then
  userdel moidify 2>/dev/null || warn "Could not remove user (maybe still in use)"
  ok "Removed system user"
fi

# Optional: remove data
read -r -p "Remove all data (music database, covers) at /var/lib/moidify? [y/N] " RM_DATA
if [[ "$RM_DATA" =~ ^[Yy]$ ]]; then
  rm -rf /var/lib/moidify
  ok "Removed /var/lib/moidify"
else
  info "Data kept at /var/lib/moidify"
fi

# Optional: remove ffmpeg (installed by installer if it wasn't present)
if command -v ffmpeg &>/dev/null; then
  read -r -p "Remove ffmpeg (audio transcoding)? [y/N] " RM_FFMPEG
  if [[ "$RM_FFMPEG" =~ ^[Yy]$ ]]; then
    if command -v apt &>/dev/null; then
      apt remove -y ffmpeg 2>/dev/null && ok "ffmpeg removed." || warn "Could not remove ffmpeg"
    elif command -v dnf &>/dev/null; then
      dnf remove -y ffmpeg 2>/dev/null && ok "ffmpeg removed." || warn "Could not remove ffmpeg"
    elif command -v pacman &>/dev/null; then
      pacman -R --noconfirm ffmpeg 2>/dev/null && ok "ffmpeg removed." || warn "Could not remove ffmpeg"
    elif command -v zypper &>/dev/null; then
      zypper remove -y ffmpeg 2>/dev/null && ok "ffmpeg removed." || warn "Could not remove ffmpeg"
    elif command -v apk &>/dev/null; then
      apk del ffmpeg 2>/dev/null && ok "ffmpeg removed." || warn "Could not remove ffmpeg"
    else
      warn "Unknown package manager — remove ffmpeg manually"
    fi
  fi
fi

echo ""
ok "${GREEN}Moidify has been uninstalled.${NC}"
echo ""
echo -e "  ${YELLOW}To remove system dependencies (python3, pip, sqlite3, git, curl, rsync):${NC}"
echo -e "  These may be used by other software — review before removing."
echo -e "  Check what depends on them first with: apt depends <package>"
