#!/usr/bin/env bash
# Command: rescan
cmd_register "rescan" "Maintenance" "Rescan music folder" "rescan [--clean]"
cmd_rescan() {
  CLEAN=false
  for arg in "$@"; do
    if [[ "$arg" == "--clean" ]]; then CLEAN=true; break; fi
  done
  TOKEN_FILE="$DATA_DIR/.cli_token"
  TOKEN=""
  if [[ -f "$TOKEN_FILE" ]]; then
    TOKEN=$(cat "$TOKEN_FILE")
  else
    info "No CLI token found. Logging in as admin..."
    read -r -p "  Admin username: " ADMIN_USER </dev/tty
    read -r -s -p "  Admin password: " ADMIN_PASS </dev/tty
    echo ""
    LOGIN=$(curl -s http://localhost:${SERVER_PORT}/api/auth/login -X POST -H 'Content-Type: application/json' -d "{\"username\":\"$ADMIN_USER\",\"password\":\"$ADMIN_PASS\"}" 2>/dev/null)
    TOKEN=$(echo "$LOGIN" | python3 -c "import sys,json; print(json.load(sys.stdin).get('token',''))" 2>/dev/null || echo "")
    if [[ -z "$TOKEN" ]]; then
      err "Login failed."
      exit 1
    fi
    echo "$TOKEN" > "$TOKEN_FILE"
    chmod 600 "$TOKEN_FILE"
  fi
  if $CLEAN; then
    info "Starting clean rescan (will remove dead tracks + refresh metadata)..."
  else
    info "Starting rescan..."
  fi
  RESULT=$(curl -s http://localhost:${SERVER_PORT}/api/admin/rescan -X POST -H 'Content-Type: application/json' -H "token: $TOKEN" -d "{\"clean\":$CLEAN}" 2>/dev/null)
  STATE=$(echo "$RESULT" | python3 -c "import sys,json; d=json.load(sys.stdin); print('running' if d.get('running') else 'idle')" 2>/dev/null || echo "unknown")
  echo -e "  Status: ${STATE}"
  echo -e "  Files found: $(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('files_found',0))" 2>/dev/null || echo "?")"
  echo -e "  Imported: $(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('files_imported',0))" 2>/dev/null || echo "?")"
  ERRORS=$(echo "$RESULT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(len(d.get('errors',[])))" 2>/dev/null || echo "0")
  if [[ "$ERRORS" != "0" ]]; then
    echo -e "  ${YELLOW}Errors: $ERRORS${NC}"
    echo "$RESULT" | python3 -c "import sys,json; [print('    - '+e) for e in json.load(sys.stdin).get('errors',[])]" 2>/dev/null || true
  fi
  ok "Rescan completed."
}
