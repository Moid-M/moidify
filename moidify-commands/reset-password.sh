#!/usr/bin/env bash
# Command: reset-password
cmd_register "reset-password" "Maintenance" "Reset admin password"
cmd_reset_password() {
  if [[ $EUID -ne 0 ]]; then
    err "This command must be run as root (sudo)."
    exit 1
  fi
  DB_PATH=$(python3 -c "
import json
try:
    with open('$CONFIG_DIR/config.json') as f:
        cfg = json.load(f)
    print(cfg.get('db_path', '$DATA_DIR/music.db'))
except Exception:
    print('$DATA_DIR/music.db')
" 2>/dev/null || echo "$DATA_DIR/music.db")
  if [[ ! -f "$DB_PATH" ]]; then
    err "Database not found at $DB_PATH"
    exit 1
  fi
  echo ""
  info "Reset admin password for Moidify"
  echo ""
  while true; do
    read -r -s -p "  New admin password: " ADMIN_PASS </dev/tty
    echo ""
    if [[ -z "$ADMIN_PASS" ]]; then
      err "Password cannot be empty"; continue
    fi
    if [[ ${#ADMIN_PASS} -lt 8 ]]; then
      err "Password must be at least 8 characters"; continue
    fi
    if ! [[ "$ADMIN_PASS" =~ [a-z] ]]; then
      err "Password must contain a lowercase letter"; continue
    fi
    if ! [[ "$ADMIN_PASS" =~ [A-Z] ]]; then
      err "Password must contain an uppercase letter"; continue
    fi
    if ! [[ "$ADMIN_PASS" =~ [0-9] ]]; then
      err "Password must contain a digit"; continue
    fi
    read -r -s -p "  Confirm password: " ADMIN_PASS2 </dev/tty
    echo ""
    if [[ "$ADMIN_PASS" == "$ADMIN_PASS2" ]]; then
      break
    fi
    err "Passwords do not match, try again."
  done
  NEW_SALT=$(python3 -c "import secrets; print(secrets.token_hex(16))")
  NEW_HASH=$(DB_PATH="$DB_PATH" NEW_SALT="$NEW_SALT" NEW_PASS="$ADMIN_PASS" python3 -c "
import hashlib, os
p = os.environ['NEW_PASS']
s = os.environ['NEW_SALT']
print(hashlib.pbkdf2_hmac('sha256', p.encode(), s.encode(), 600000).hex())
")
  DB_PATH="$DB_PATH" NEW_HASH="$NEW_HASH" NEW_SALT="$NEW_SALT" python3 -c "
import sqlite3, sys, os

conn = sqlite3.connect(os.environ['DB_PATH'])
admin = conn.execute('SELECT id, username FROM users WHERE is_admin = 1 LIMIT 1').fetchone()
if not admin:
    print('No admin user found in database.')
    sys.exit(1)

conn.execute('UPDATE users SET password_hash = ?, salt = ? WHERE id = ?',
    (os.environ['NEW_HASH'], os.environ['NEW_SALT'], admin[0]))
conn.commit()
conn.close()
print(f'Updated password for admin user \"{admin[1]}\" (id={admin[0]})')
"
  ok "Admin password reset successfully."
}
