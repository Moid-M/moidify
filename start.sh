#!/usr/bin/env bash
set -e
cd "$(dirname "$0")"
export PATH="$HOME/.local/bin:$PATH"

REQUIRED="fastapi uvicorn mutagen watchdog python_multipart"

check_deps() {
  python3 -c "
import sys
missing = []
for pkg in '$REQUIRED'.split():
    try:
        __import__(pkg.replace('_', '-').split('[')[0].replace('-', '_'))
    except ImportError:
        missing.append(pkg)
if missing:
    print('MISSING:' + ','.join(missing))
    sys.exit(1)
else:
    print('OK')
" 2>/dev/null
}

if ! python3 --version &>/dev/null; then
  echo "Error: Python 3 is required. Install it with: apt install python3"
  exit 1
fi

DEPS_CHECK=$(check_deps)
if [ "$DEPS_CHECK" != "OK" ]; then
  echo "Installing missing dependencies..."
  MISSING=$(echo "$DEPS_CHECK" | sed 's/^MISSING://')

  # Install pip if missing
  python3 -m pip --version &>/dev/null || python3 <(curl -sS https://bootstrap.pypa.io/get-pip.py) --break-system-packages --user 2>/dev/null

  pip install --user --break-system-packages -r requirements.txt 2>&1 | tail -3

  # Verify installation
  DEPS_CHECK=$(check_deps)
  if [ "$DEPS_CHECK" != "OK" ]; then
    echo "Error: Failed to install: $DEPS_CHECK"
    echo "Try running: pip install --user --break-system-packages -r requirements.txt"
    exit 1
  fi
  echo "Dependencies installed."
fi

echo "Starting Moidify..."
echo "Open http://localhost:8000 in your browser"
nohup python3 server.py > /tmp/moidify.log 2>&1 &
disown
echo "Server PID: $!"
