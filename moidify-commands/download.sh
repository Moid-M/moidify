#!/usr/bin/env bash
# Command: download
cmd_register "download" "Import" "Download a song/video by URL" "download [--format mp3|flac|opus|aac|wav] <url>"
cmd_download() {
  # Parse --format flag and collect URL
  DOWNLOAD_FMT="mp3"
  URL=""
  NEXT_IS_FMT=false
  for arg in "$@"; do
    if $NEXT_IS_FMT; then DOWNLOAD_FMT="$arg"; NEXT_IS_FMT=false; continue; fi
    if [[ "$arg" == "--format" ]]; then NEXT_IS_FMT=true; continue; fi
    if [[ "$arg" == --format=* ]]; then DOWNLOAD_FMT="${arg#*=}"; continue; fi
    URL="${URL}${URL:+ }${arg}"
  done
  if [[ -z "$URL" ]]; then
    err "Usage: moidify download [--format mp3|flac|opus|aac|wav] <url>"
    echo ""
    echo "Download a song from YouTube/SoundCloud/etc and import it into your library."
    echo ""
    echo -e "${YELLOW}Options:${NC}"
    echo -e "  ${GREEN}--format${NC}  Output format (default: mp3)"
    echo -e "              ${YELLOW}mp3, flac, opus, aac, wav${NC}"
    echo ""
    echo -e "${YELLOW}Tip:${NC} Quote the URL if it contains ${YELLOW}&${NC} or other special characters:"
    echo -e "  moidify download ${YELLOW}'${NC}https://...${YELLOW}'${NC}"
    exit 1
  fi
  # Use an API token from the CLI config for admin access
  TOKEN_FILE="$DATA_DIR/.cli_token"
  TOKEN=""
  if [[ -f "$TOKEN_FILE" ]]; then
    TOKEN=$(cat "$TOKEN_FILE")
  else
    # Try to login with admin credentials from config prompt
    info "No CLI token found. Logging in as admin..."
    read -r -p "  Admin username: " ADMIN_USER </dev/tty
    read -r -s -p "  Admin password: " ADMIN_PASS </dev/tty
    echo ""
    LOGIN=$(curl -s http://localhost:${SERVER_PORT}/api/auth/login -X POST -H 'Content-Type: application/json' -d "{\"username\":\"$ADMIN_USER\",\"password\":\"$ADMIN_PASS\"}" 2>/dev/null)
    TOKEN=$(echo "$LOGIN" | python3 -c "import sys,json; print(json.load(sys.stdin).get('token',''))" 2>/dev/null || echo "")
    if [[ -z "$TOKEN" ]]; then
      err "Login failed. Check your credentials."
      exit 1
    fi
    echo "$TOKEN" > "$TOKEN_FILE"
    chmod 600 "$TOKEN_FILE"
    ok "Logged in. Token cached."
  fi
  info "Submitting download: $URL (format: $DOWNLOAD_FMT)"
  JOB=$(curl -s http://localhost:${SERVER_PORT}/api/admin/import-url -X POST -H 'Content-Type: application/json' -H "token: $TOKEN" -d "{\"url\":\"$URL\",\"format\":\"$DOWNLOAD_FMT\"}" 2>/dev/null)
  JOB_ID=$(echo "$JOB" | python3 -c "import sys,json; print(json.load(sys.stdin).get('job_id',''))" 2>/dev/null || echo "")
  if [[ -z "$JOB_ID" ]]; then
    err "Failed to submit download. Response: $JOB"
    rm -f "$TOKEN_FILE"
    exit 1
  fi
  info "Download job: $JOB_ID"
  # Poll for progress
  while true; do
    STATUS=$(curl -s http://localhost:${SERVER_PORT}/api/admin/import-status/${JOB_ID} -H "token: $TOKEN" 2>/dev/null)
    STATE=$(echo "$STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
    PROGRESS=$(echo "$STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin).get('progress',0))" 2>/dev/null || echo "0")
    ERROR=$(echo "$STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error',''))" 2>/dev/null || echo "")
    if [[ "$STATE" == "done" ]]; then
      TITLE=$(echo "$STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin).get('title','Unknown'))" 2>/dev/null || echo "Unknown")
      ok "Download complete: $TITLE"
      break
    elif [[ "$STATE" == "error" ]]; then
      err "Download failed: $ERROR"
      exit 1
    elif [[ "$STATE" == "downloading" ]]; then
      echo -ne "\r  Progress: ${PROGRESS}%    "
    elif [[ "$STATE" == "importing" ]]; then
      echo -ne "\r  Importing to library...    "
    fi
    sleep 2
  done
  echo ""
}
