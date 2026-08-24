#!/usr/bin/env bash
# Command: update
cmd_register "update" "Maintenance" "Run update script"
cmd_update() {
  if [[ -f "$APP_DIR/update.sh" ]]; then
    exec "$APP_DIR/update.sh"
  else
    err "Update script not found at $APP_DIR/update.sh"
    exit 1
  fi
}
