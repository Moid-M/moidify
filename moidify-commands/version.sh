#!/usr/bin/env bash
# Command: version
cmd_register "version" "Info" "Show installed version"
cmd_version() {
  echo "Moidify $(version)"
}
