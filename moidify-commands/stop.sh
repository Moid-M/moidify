#!/usr/bin/env bash
# Command: stop
cmd_register "stop" "Service" "Stop moidify service"
cmd_stop() {
  info "Stopping moidify..."
  if systemctl stop "$SERVICE" 2>/dev/null; then
    ok "Moidify stopped."
  else
    pkill -f "uvicorn.*server:app" 2>/dev/null && ok "Moidify stopped." || warn "No running process found."
  fi
}
