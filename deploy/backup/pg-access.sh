# shellcheck shell=bash

ANIMA_DB_NAME="${ANIMA_DB_NAME:-anima}"
ANIMA_DB_OWNER="${ANIMA_DB_OWNER:-anima_owner}"
ANIMA_DB_SUPERUSER="${ANIMA_DB_SUPERUSER:-postgres}"
ANIMA_COMPOSE_FILE="${ANIMA_COMPOSE_FILE:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/compose.yaml}"
export ANIMA_DB_NAME ANIMA_DB_OWNER ANIMA_DB_SUPERUSER

pg_run() {
    if [ "${ANIMA_PG_LOCAL:-0}" = 1 ]; then
        "$@"
    else
        docker compose -f "$ANIMA_COMPOSE_FILE" exec -T postgres "$@"
    fi
}

fail() {
    printf '%s: %s\n' "$(basename "$0")" "$*" >&2
    exit 1
}

superuser_sql() { pg_run psql -X -q -v ON_ERROR_STOP=1 -U "$ANIMA_DB_SUPERUSER" -d postgres "$@"; }

# It selects no column, so no row content can reach the output.
counts_query() {
    cat <<'SQL'
select t || ' ' || (xpath('/row/c/text()',
        query_to_xml(format('select count(*) as c from %s', t), false, true, '')))[1]::text
from (select format('%I.%I', schemaname, tablename) as t from pg_tables
      where schemaname not in ('pg_catalog', 'information_schema')) tables
order by 1;
SQL
}

count_rows() {
    counts_query | pg_run psql -X -q -At -v ON_ERROR_STOP=1 -U "$ANIMA_DB_OWNER" -d "$1"
}

# Matches the grants of deploy/postgres/roles.sql by hand: roles.sql also makes roles, so it cannot be included.
create_database() {
    superuser_sql -v db="$1" -v owner="$ANIMA_DB_OWNER" <<'SQL'
set client_min_messages = warning;
drop database if exists :"db" with (force);
create database :"db" owner :"owner";
revoke connect, temporary on database :"db" from public;
grant connect on database :"db" to anima_runtime;
SQL
}

drop_database() {
    superuser_sql -c "set client_min_messages = warning" -c "drop database if exists \"$1\" with (force)" ||
        printf '%s: warning: could not drop database %s; drop it by hand\n' "$(basename "$0")" "$1" >&2
}

require_readable_dump() {
    pg_run pg_restore --list <"$1" >/dev/null || fail "$1 is not a readable dump"
}

restore_into() {
    pg_run pg_restore --exit-on-error --single-transaction \
        -U "$ANIMA_DB_OWNER" -d "$1" <"$2" || fail "pg_restore failed"
}
