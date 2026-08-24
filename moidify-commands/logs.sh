#!/usr/bin/env bash
# Command: logs
cmd_register "logs" "Info" "Follow service logs (Ctrl+C to exit)"
cmd_logs() {
  if command -v journalctl &>/dev/null && systemctl is-active --quiet "$SERVICE" 2>/dev/null; then
    exec journalctl -u "$SERVICE" -f -n 30 --no-pager
  else
    exec tail -f /tmp/moidify.log 2>/dev/null || err "No log file found."
  fi
}
