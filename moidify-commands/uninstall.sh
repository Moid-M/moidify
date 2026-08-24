#!/usr/bin/env bash
# Command: uninstall
cmd_register "uninstall" "Maintenance" "Uninstall Moidify (type 'KoRn' to confirm)"
cmd_uninstall() {
  if [[ $EUID -ne 0 ]]; then
    err "This command must be run as root (sudo)."
    exit 1
  fi
  echo ""
  warn "This will remove Moidify and ALL data (music, database, covers)."
  echo ""
  read -r -p "  Type 'KoRn' (exact upper/lowercase) to confirm uninstall: " CONFIRM
  echo ""
  if [[ "$CONFIRM" != "KoRn" ]]; then
    err "Confirmation did not match 'KoRn'. Aborting — nothing was removed."
    exit 1
  fi
  if [[ -f "$APP_DIR/uninstall.sh" ]]; then
    exec "$APP_DIR/uninstall.sh"
  else
    err "Uninstall script not found at $APP_DIR/uninstall.sh"
    exit 1
  fi
}
