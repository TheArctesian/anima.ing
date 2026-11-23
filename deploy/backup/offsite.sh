#!/usr/bin/env bash
# Usage: offsite.sh [<dump>]; sends a dump and its counts manifest to the home PC (VPS-006).
# The repository is append-only for this host: pruning happens on the home PC.
set -euo pipefail
umask 077

fail() {
    printf 'offsite.sh: %s\n' "$*" >&2
    exit 1
}

secrets="${ANIMA_SECRETS_DIR:-/opt/anima/deploy/secrets}"
repository_file="${ANIMA_RESTIC_REPOSITORY_FILE:-$secrets/restic_repository}"
password_file="${ANIMA_RESTIC_PASSWORD_FILE:-$secrets/restic_password}"
backup_dir="${ANIMA_BACKUP_DIR:-/var/backups/anima}"
restic="${RESTIC:-restic}"

dump="${1:-}"
if [ -z "$dump" ]; then
    # File names hold the UTC time, so the last name in sorted order is the newest.
    for candidate in "$backup_dir"/anima-*; do
        [[ $(basename "$candidate") =~ ^anima-[0-9]{8}T[0-9]{6}Z\.dump$ ]] && dump="$candidate"
    done
fi
[ -n "$dump" ] || fail "no dump in $backup_dir"
[ -f "$dump" ] || fail "no such file: $dump"
[ -f "$dump.counts" ] || fail "the manifest $dump.counts is missing"
[ -s "$repository_file" ] || fail "repository file $repository_file is missing or empty"
[ -s "$password_file" ] || fail "password file $password_file is missing or empty"

cache=()
if [ -n "${ANIMA_RESTIC_CACHE_DIR:-}" ]; then cache=(--cache-dir "$ANIMA_RESTIC_CACHE_DIR"); fi

"$restic" ${cache[@]+"${cache[@]}"} --repository-file "$repository_file" --password-file "$password_file" \
    backup --host anima-vps --tag anima-db "$dump" "$dump.counts" >/dev/null ||
    fail "restic backup failed (no snapshot is guaranteed)"
echo "Sent $dump to the home PC."
