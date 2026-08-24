#!/usr/bin/env bash
# Command: enable
cmd_register "enable" "Service" "Enable service on boot"
cmd_enable() {
  systemctl enable "$SERVICE" 2>/dev/null && ok "Service enabled on boot." || err "Failed to enable service."
}
