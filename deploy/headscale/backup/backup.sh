#!/usr/bin/env bash
# Daily Headscale backup to the rest-server on the home Linux PC (VPS-006, D-12).
# restic reads the repository and the password from files only, never from arguments or the environment.
# The repository is append-only for the VPS; pruning runs on the home PC (prune.sh).
set -euo pipefail

readonly SQLITE3="${SQLITE3:-sqlite3}"
readonly RESTIC="${RESTIC:-restic}"
readonly HEADSCALE_DIR="${HEADSCALE_DIR:-/opt/anima/headscale}"
readonly CREDENTIALS="${CREDENTIALS_DIRECTORY:-}"
readonly REPOSITORY_FILE="${RESTIC_REPOSITORY_FILE:-${CREDENTIALS:+$CREDENTIALS/restic-repository}}"
readonly PASSWORD_FILE="${RESTIC_PASSWORD_FILE:-${CREDENTIALS:+$CREDENTIALS/restic-password}}"
readonly LIB="$HEADSCALE_DIR/lib"
readonly CONFIG="$HEADSCALE_DIR/config"

unset RESTIC_REPOSITORY RESTIC_PASSWORD RESTIC_PASSWORD_COMMAND RESTIC_REPOSITORY_FILE RESTIC_PASSWORD_FILE

fail() {
  printf 'headscale-backup: %s\n' "$*" >&2
  exit 1
}

require_secret_file() {
  local what="$1" path="$2"
  [[ -n "$path" ]] || fail "$what is not set (set it or run under systemd with LoadCredential=)"
  [[ -r "$path" && -s "$path" ]] || fail "$what is missing, empty or unreadable: $path"
}

require_secret_file "restic repository file" "$REPOSITORY_FILE"
require_secret_file "restic password file" "$PASSWORD_FILE"
[[ -r "$LIB/db.sqlite" ]] || fail "Headscale database not readable: $LIB/db.sqlite"

if [[ -n "${CACHE_DIRECTORY:-}" ]]; then
  cache=(--cache-dir "$CACHE_DIRECTORY")
else
  cache=(--no-cache)
fi

restic_run() {
  "$RESTIC" --repository-file "$REPOSITORY_FILE" --password-file "$PASSWORD_FILE" "${cache[@]}" "$@"
}

umask 077
# BSD mktemp ignores TMPDIR without a template.
staging="$(mktemp -d "${TMPDIR:-/tmp}/headscale-backup.XXXXXXXX")" || fail "could not create a private temporary folder"
# The snapshot holds the Noise and DERP private keys.
trap 'rm -rf -- "$staging"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
[[ "$staging" != *"'"* ]] || fail "temporary folder path contains a quote: $staging"

# VACUUM INTO takes a consistent copy of a live WAL-mode database.
"$SQLITE3" -batch -bail -readonly "$LIB/db.sqlite" "VACUUM INTO '$staging/db.sqlite'" \
  || fail "sqlite3 VACUUM INTO failed for $LIB/db.sqlite"
integrity="$("$SQLITE3" -batch -bail -readonly "$staging/db.sqlite" "PRAGMA integrity_check")" \
  || fail "sqlite3 integrity_check could not run on the snapshot"
[[ "$integrity" == "ok" ]] || fail "integrity_check of the database snapshot did not return ok"

for file in "$LIB/noise_private.key" "$LIB/derp_server_private.key" \
  "$CONFIG/config.yaml" "$CONFIG/policy.hujson"; do
  cp -- "$file" "$staging/" || fail "could not copy $file"
done

# restic exits 0 only when the snapshot is saved; 3 (some files unreadable) is a failure here.
restic_run backup --host anima-vps --tag headscale "$staging" \
  || fail "restic backup failed (no snapshot is guaranteed)"
printf 'headscale-backup: snapshot sent\n'
