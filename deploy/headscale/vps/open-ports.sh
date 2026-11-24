# shellcheck shell=bash
# Which ports are open to the internet, and the guard that refuses to close one silently (VPS-002, VPS-005).

ALLOWED_PUBLIC=" 22/tcp 443/tcp 3478/udp 41641/udp "
LOCAL_ALLOW=/etc/anima/firewall-allow

is_vpn_address() {
  local address=${1#::ffff:} a b
  case $address in
    *:*) case $(tr 'A-F' 'a-f' <<<"$address") in fd7a:115c:a1e0:*) return 0 ;; esac; return 1 ;;
  esac
  [[ $address =~ ^[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}$ ]] || return 1
  IFS=. read -r a b _ <<<"$address"
  [ "$a" -eq 100 ] && [ "$b" -ge 64 ] && [ "$b" -le 127 ]
}

local_accepts() {
  local file line
  file=$(path "$LOCAL_ALLOW")
  [ -f "$file" ] || return 0
  while IFS= read -r line || [ -n "$line" ]; do
    line=${line%%#*}
    line=${line//[[:space:]]/}
    [ -n "$line" ] || continue
    if ! [[ $line =~ ^([1-9][0-9]{0,4})/(tcp|udp)$ ]] || [ "${BASH_REMATCH[1]}" -gt 65535 ]; then
      fail "$LOCAL_ALLOW: not one PORT/PROTO (tcp or udp): $line"
    fi
    [ "$line" != 22/tcp ] || fail "$LOCAL_ALLOW: 22/tcp stays on the VPN only (VPS-001)"
    say "$line"
  done <"$file"
}

kept_open() {
  case "$ALLOWED_PUBLIC$(local_accepts | tr '\n' ' ') " in *" $1 "*) return 0 ;; esac
  return 1
}

public_sockets() {
  local sockets netid local_address rest port host scope
  sockets=$($SS -Htulpn) || return 1
  while read -r netid _ _ _ local_address _ rest; do
    [ -n "${local_address:-}" ] || continue
    port=${local_address##*:}
    host=${local_address%:*}
    host=${host#[}
    host=${host%]}
    scope=
    case $host in *%*) scope=${host#*%}; host=${host%%\%*} ;; esac
    case $scope in lo | tailscale0) continue ;; esac
    case $host in 127.* | ::1) continue ;; esac
    if is_vpn_address "$host"; then continue; fi
    say "$port/$netid $netid $local_address $rest"
  done <<<"$sockets"
}

# Interfaces, priorities, intra-zone forwarding and ICMP blocks let nothing in from outside.
zone_entries() {
  $FIREWALL_CMD --zone="$1" --list-all | awk -v zone="$1" '
    /^\t/ { if (key != "") value[key] = value[key] " " $0; next }
    /^[[:space:]]+[a-z][a-z -]*:/ {
      key = $0; sub(/^[[:space:]]+/, "", key); sub(/:.*/, "", key)
      value[key] = $0; sub(/^[^:]*:/, "", value[key]); order[++n] = key
    }
    END {
      for (i = 1; i <= n; i++) {
        k = order[i]; v = value[k]; gsub(/^[[:space:]]+|[[:space:]]+$/, "", v)
        if (k ~ /^(interfaces|ingress-priority|egress-priority|forward|icmp-blocks)$/) continue
        if (k == "ports" || k == "services") {
          c = split(v, words, /[[:space:]]+/)
          for (j = 1; j <= c; j++) if (words[j] != "") print (k == "ports" ? "" : "service ") words[j]
          continue
        }
        if (k == "target" && v == "default") continue
        if (k ~ /^(masquerade|icmp-block-inversion)$/ && v == "no") continue
        if (v != "") { gsub(/ /, "-", k); print zone ":" k }
      }
    }'
}

service_openings() {
  local service_info port
  service_info=$($FIREWALL_CMD --info-service="$2") || { say "service:$2 firewalld zone $1 (unknown service)"; return; }
  while read -r port; do
    say "$port firewalld zone $1 service $2"
  done < <(awk '$1 == "ports:" { for (i = 2; i <= NF; i++) print $i }' <<<"$service_info")
  if awk '$1 ~ /^(protocols|source-ports|includes):$/ && NF > 1 { found = 1 } END { exit !found }' <<<"$service_info"; then
    say "service:$2 firewalld zone $1 (protocols, source ports or included services)"
  fi
}

# The stock `--info-policy` output of firewalld 2.4.3 on AlmaLinux 10, without the priority line.
stock_policy() {
  case $1 in
    allow-host-ipv6)
      printf '%s\n' allow-host-ipv6 '  disable: no' '  target: CONTINUE' '  ingress-zones: ANY' \
        '  egress-zones: HOST'
      printf '\trule family="ipv6" icmp-type name="%s" accept\n' mld-listener-done mld-listener-query \
        mld-listener-report mld2-listener-report neighbour-advertisement neighbour-solicitation \
        redirect router-advertisement ;;
    docker-forwarding)
      printf '%s\n' docker-forwarding '  disable: no' '  target: ACCEPT' '  ingress-zones: ANY' \
        '  egress-zones: docker' ;;
    *) return 0 ;;
  esac
  printf '%s\n' '  services:' '  ports:' '  protocols:' '  masquerade: no' '  forward-ports:' \
    '  source-ports:' '  icmp-blocks:' '  rich rules:'
}

# The running firewall-cmd adds " (active)" and firewall-offline-cmd does not; Docker sets its own priority.
policy_form() {
  sed -e 's/[[:space:]]*$//' -e '1s/ (active)$//' -e '/^  priority:/d' | LC_ALL=C sort
}

stock_policy_in_force() {
  local shown
  [ -n "$(stock_policy "$1")" ] || return 1
  shown=$($FIREWALL_CMD --info-policy="$1") || return 1
  if [ "$1" = allow-host-ipv6 ]; then grep -qx '  priority: -15000' <<<"$shown" || return 1; fi
  [ "$(policy_form <<<"$shown")" = "$(stock_policy "$1" | policy_form)" ]
}

firewalld_extras() {
  local policy
  while read -r policy _; do
    [ -n "$policy" ] || continue
    if stock_policy_in_force "$policy"; then continue; fi
    say "policy:$policy firewalld policy $policy"
  done < <($FIREWALL_CMD --get-active-policies | grep -v '^[[:space:]]')
  if [ -n "$($FIREWALL_CMD --direct --get-all-rules)$($FIREWALL_CMD --direct --get-all-chains)$($FIREWALL_CMD --direct --get-all-passthroughs)" ]; then
    say "direct-rules firewalld direct rules, chains or passthroughs"
  fi
}

# Tokens for anything else that could let traffic in, because no port check can see through them.
firewalld_openings() {
  local zones zone entry
  $SYSTEMCTL is-active --quiet firewalld.service || return 0
  zones=$($FIREWALL_CMD --get-active-zones) || { say "unknown firewall-cmd --get-active-zones failed"; return; }
  while read -r zone _; do
    # Docker's own zone holds docker0 (target ACCEPT); it is not an opening to the internet.
    case $zone in '' | docker) continue ;; esac
    while read -r entry; do
      case $entry in
        '') ;;
        "service "*) service_openings "$zone" "${entry#service }" ;;
        *) say "$entry firewalld zone $zone" ;;
      esac
    done < <(zone_entries "$zone")
  done < <(grep -v '^[[:space:]]' <<<"$zones")
  firewalld_extras
}

listener_violations() {
  local sockets port socket
  sockets=$(public_sockets) || { say "ss -Htulpn failed"; return; }
  while read -r port socket; do
    [ -n "$port" ] || continue
    kept_open "$port" || say "public listener $socket"
  done <<<"$sockets"
}

# SSH (22/tcp) moves to the VPN on purpose (VPS-001).
closing_ports() {
  local sockets port what
  sockets=$(public_sockets) || { say "unknown (ss -Htulpn failed)"; return; }
  while read -r port what; do
    case $port in '' | 22/tcp) continue ;; esac
    if kept_open "$port"; then continue; fi
    case ",$ALLOW_CLOSING," in *",$port,"*) continue ;; esac
    # Docker's DNAT runs before the input chain, so the Anima table cannot close a port Docker publishes.
    case $what in
      *'(("docker-proxy",'* | *'(("dockerd",'*)
        say "note: $port ($what) is published by Docker, not closed by this firewall; see guide 4.4" >&2
        continue ;;
    esac
    say "$port ($what)"
  done < <(printf '%s\n' "$sockets"; firewalld_openings)
}

refuse_closing() {
  local closing line
  closing=$(closing_ports)
  [ -n "$closing" ] || return 0
  while IFS= read -r line; do say "would close: $line"; done <<<"$closing" >&2
  refuse "the Anima firewall would close the ports above. Keep one open with a line PORT/PROTO in $LOCAL_ALLOW, or close it on purpose with --allow-closing PORT/PROTO,..."
}
