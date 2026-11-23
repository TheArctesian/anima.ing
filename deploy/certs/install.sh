#!/bin/sh
# lego hook (DNS-002, SEC-001): writes the key and the chain as one PEM file that the hub reads for both.
# Root without capabilities cannot read the 0400 file it gave the hub user, so a stamp holds its checksum and stat.
# Under cap_drop ALL root may chmod only files it owns, so chmod comes before chown.
set -eu
umask 077

cert=${LEGO_CERT_PATH:?install.sh: LEGO_CERT_PATH is not set}
key=${LEGO_CERT_KEY_PATH:?install.sh: LEGO_CERT_KEY_PATH is not set}
dest=${ANIMA_CERTS_DIR:-/certs/hub}
owner=${ANIMA_CERTS_OWNER-10001:10001}
stamp=${ANIMA_CERTS_STAMP:-/var/lib/lego/tls.pem.sha256}

mkdir -p "$dest"
chmod 0755 "$dest"

file_state() {
    stat -c '%i %s %Y %u %a' "$1" 2>/dev/null || stat -f '%i %z %m %u %Lp' "$1"
}

stamp_text() {
    printf '%s\n%s' "$1" "$(file_state "$dest/tls.pem")"
}

tmp=$(mktemp "$dest/.tls.pem.XXXXXX")
trap 'rm -f "$tmp"' EXIT
cat "$key" "$cert" >"$tmp"

sum=$(sha256sum <"$tmp" 2>/dev/null || shasum -a 256 <"$tmp")
if [ -f "$dest/tls.pem" ] && [ -f "$stamp" ] && [ "$(cat "$stamp")" = "$(stamp_text "$sum")" ]; then
    echo "install.sh: $dest/tls.pem is up to date"
    exit 0
fi

chmod 0400 "$tmp"
if [ -n "$owner" ]; then
    chown "$owner" "$tmp"
fi
mv "$tmp" "$dest/tls.pem"
trap - EXIT
stamp_text "$sum" >"$stamp"
echo "install.sh: installed $dest/tls.pem"
