#!/usr/bin/env bash
# Command: reload (alias of restart)
cmd_register "reload" "Maintenance" "Reload (restart) service"
cmd_reload() {
  cmd_restart "$@"
}
