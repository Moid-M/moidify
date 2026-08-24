#!/usr/bin/env bash
# Command: restart
cmd_register "restart" "Service" "Restart moidify service"
cmd_restart() {
  info "Restarting moidify..."
  if systemctl restart "$SERVICE" 2>/dev/null; then
    sleep 1
    if systemctl is-active --quiet "$SERVICE" 2>/dev/null; then
      ok "Moidify restarted → $(url)"
    else
      warn "Check logs: moidify logs"
    fi
  else
    pkill -f "uvicorn.*server:app" 2>/dev/null || true
    if [[ -f "$APP_DIR/server.py" ]]; then
      cd "$APP_DIR"
      nohup "$APP_DIR/venv/bin/uvicorn" server:app --host "$SERVER_HOST" --port "$SERVER_PORT" > /tmp/moidify.log 2>&1 &
      disown
      sleep 1
      ok "Moidify restarted → $(url)"
    fi
  fi
}
