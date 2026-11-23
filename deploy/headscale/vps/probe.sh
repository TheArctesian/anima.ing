# shellcheck shell=bash
# `harden.sh probe` (VP-13, VPS-001): anima-probe, the hub's SSH user, whose uid the firewall's output chain confines.

PROBE_ID=10002
PROBE_HOME=/home/$PROBE_USER
PROBE_KEY_OPTIONS="restrict,pty,from=\"${PROBE_ALLOW#*@}\""
# The type comes first, so the line carries no options.
PROBE_KEY_START='^(ssh-|ecdsa-|sk-)'

# The line is printed nowhere else, not even in a refusal.
probe_key_line() {
  local file=$1 lines line
  [ -n "$file" ] || refuse "pass --key=FILE, the hub's public key (anima-probe.pub)"
  [ -f "$file" ] || refuse "$file is not a file"
  lines=$(awk 'NF { n++ } END { print n + 0 }' "$file")
  [ "$lines" = 1 ] || refuse "$file must hold exactly one public key line"
  line=$(awk 'NF' "$file")
  if ! [[ $line =~ $KEY_LINE_PATTERN && $line =~ $PROBE_KEY_START ]] || [[ $line =~ [[:cntrl:]] ]]; then
    refuse "$file is not an OpenSSH public key line without options (ssh-keygen's .pub file)"
  fi
  printf '%s' "$line"
}

probe_account() {
  local db entry line fields
  for db in group passwd; do
    for entry in "$PROBE_ID" "$PROBE_USER"; do
      line=$($GETENT "$db" "$entry") || continue
      IFS=: read -r -a fields <<<"$line"
      [ "${fields[0]}" = "$PROBE_USER" ] && [ "${fields[2]}" = "$PROBE_ID" ] \
        || refuse "$db $entry is ${fields[0]}:${fields[2]}; $PROBE_USER needs $PROBE_ID, the uid the firewall confines"
      [ "$db" = group ] || [ "${fields[3]}:${fields[5]}" = "$PROBE_ID:$PROBE_HOME" ] \
        || refuse "$PROBE_USER has group ${fields[3]} and home ${fields[5]}; it needs $PROBE_ID and $PROBE_HOME"
    done
  done
  $GETENT group "$PROBE_ID" >/dev/null || run "$GROUPADD" --gid "$PROBE_ID" "$PROBE_USER"
  $GETENT passwd "$PROBE_ID" >/dev/null \
    || run "$USERADD" --uid "$PROBE_ID" --gid "$PROBE_ID" --create-home --home-dir "$PROBE_HOME" \
      --shell /bin/bash "$PROBE_USER"
}

# Runs as anima-probe, so a link the user put in its own home cannot make root write or chown anything.
probe_write_keys() {
  local home
  home=$(path "$PROBE_HOME")
  # shellcheck disable=SC2016 # the script expands $1 when it runs, as the user
  "$RUNUSER" -u "$PROBE_USER" -- sh -c 'umask 077 && mkdir -p "$1/.ssh" && chmod 0700 "$1/.ssh" &&
    tmp=$(mktemp "$1/.ssh/authorized_keys.XXXXXX") && cat >"$tmp" && chmod 0600 "$tmp" &&
    mv -f "$tmp" "$1/.ssh/authorized_keys"' sh "$home"
}

cmd_probe() {
  local line
  require_root
  line=$(probe_key_line "$PROBE_KEY") || exit $?
  probe_account
  if dry; then
    say "[dry-run] write $(path "$PROBE_HOME/.ssh/authorized_keys") as $PROBE_USER (mode 0600) with the key and $PROBE_KEY_OPTIONS"
    return
  fi
  printf '%s %s\n' "$PROBE_KEY_OPTIONS" "$line" | probe_write_keys \
    || fail "writing $PROBE_HOME/.ssh/authorized_keys as $PROBE_USER failed"
  say "$PROBE_USER has the hub's key, usable only from ${PROBE_ALLOW#*@}. Store the private key in Anima."
}

probe_sudoers() {
  local file
  for file in "$(path /etc/sudoers)" "$(path /etc/sudoers.d)"/*; do
    [ -f "$file" ] || continue
    awk -v pattern="(^|[^A-Za-z0-9_.-])($PROBE_USER|#$PROBE_ID)([^A-Za-z0-9_.-]|$)" '
      /^[ \t]*#([^0-9]|$)/ { next }
      $0 ~ pattern { found = 1 }
      END { exit !found }' "$file" && say "${file#"${ROOT%/}"} names $PROBE_USER; it may have no sudo grant"
  done
  return 0
}

probe_keys() {
  local keys=$1
  [ -f "$keys" ] || return 0
  awk -v from="from=\"${PROBE_ALLOW#*@}\"" -v user="$PROBE_USER" '
    /^[ \t]*(#|$)/ { next }
    {
      options = $1 ~ /^(ssh-|ecdsa-|sk-)/ ? "" : $1
      n = split(options, option, ",")
      restricted = 0; hub = 0; other = 0
      for (i = 1; i <= n; i++) {
        if (option[i] == "restrict") restricted = 1
        else if (option[i] == from) hub = 1
        else if (option[i] != "pty") other = 1
      }
      if (!restricted || !hub || other) print user "'"'"'s authorized_keys line " FNR " is not limited to restrict,pty," from
    }' "$keys"
}

probe_problem() {
  local entry uid home groups group shadow password
  entry=$($GETENT passwd "$PROBE_USER") || return 0
  IFS=: read -r _ _ uid _ _ home _ <<<"$entry"
  [ "$uid" = "$PROBE_ID" ] || say "$PROBE_USER has uid $uid, not $PROBE_ID: the firewall's output chain does not confine it"
  if groups=$($ID -nG "$PROBE_USER"); then
    for group in $groups; do
      [ "$group" = "$PROBE_USER" ] || say "$PROBE_USER is in group $group; it may be in its own group only"
    done
  else
    say "cannot read the groups of $PROBE_USER"
  fi
  probe_sudoers
  if shadow=$($GETENT shadow "$PROBE_USER"); then
    IFS=: read -r _ password _ <<<"$shadow"
    case $password in
      '!'* | '*'*) ;;
      *) say "$PROBE_USER has a usable password; lock it: passwd -l $PROBE_USER" ;;
    esac
  else
    say "cannot read the shadow entry of $PROBE_USER"
  fi
  probe_keys "$(path "$home/.ssh/authorized_keys")"
}
