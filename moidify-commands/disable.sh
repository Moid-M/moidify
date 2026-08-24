#!/usr/bin/env bash
# Command: disable
cmd_register "disable" "Service" "Disable service on boot"
cmd_disable() {
  systemctl disable "$SERVICE" 2>/dev/null && ok "Service disabled on boot." || err "Failed to disable service."
}
