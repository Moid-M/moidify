#!/usr/bin/env bash
# Command: status
cmd_register "status" "Service" "Show service status"
cmd_status() {
  if systemctl status "$SERVICE" 2>/dev/null; then
    exit 0
  fi
  if pgrep -f "uvicorn.*server:app" >/dev/null 2>&1; then
    echo -e "${GREEN}●${NC} Moidify is running"
    echo "   $(url)"
  else
    echo -e "${RED}●${NC} Moidify is not running"
  fi
}
