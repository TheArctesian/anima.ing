#!/bin/sh
# Entry point of the `certs` service (DNS-002, DNS-003); compose runs it with `init: true`, so a stop signal ends the loop.
set -eu

: "${ANIMA_ACME_EMAIL:?run.sh: ANIMA_ACME_EMAIL is not set}"
domain=hub.anima.ing
lego=${ANIMA_LEGO:-/lego}
state=${ANIMA_LEGO_STATE:-/var/lib/lego}
server=${ANIMA_ACME_SERVER:-https://acme-v02.api.letsencrypt.org/directory}
hook=${ANIMA_INSTALL_HOOK:-/anima/certs/install.sh}
preview_domain=
if [ -n "${ANIMA_PREVIEW_ORIGIN:-}" ]; then
    # Compose publishes HTTPS on 443. Other ingress arrangements supply their own TLS.
    case "$ANIMA_PREVIEW_ORIGIN" in
        https://*) preview_domain=${ANIMA_PREVIEW_ORIGIN#https://} ;;
        *) echo "run.sh: preview origin must be a canonical HTTPS DNS origin" >&2; exit 1 ;;
    esac
    case "$preview_domain" in
        ''|*[!a-z0-9.-]*|.*|*.|*..*|-*|*-|*.-*|*-.*)
            echo "run.sh: preview origin must be a lowercase DNS name without a port or path" >&2
            exit 1 ;;
    esac
    if [ "${#preview_domain}" -gt 216 ]; then
        echo "run.sh: preview DNS suffix is too long" >&2
        exit 1
    fi
    remaining=$preview_domain
    while :; do
        label=${remaining%%.*}
        if [ "${#label}" -gt 63 ]; then
            echo "run.sh: preview DNS label is too long" >&2
            exit 1
        fi
        case "$remaining" in
            *.*) remaining=${remaining#*.} ;;
            *) break ;;
        esac
    done
    case "$label" in
        *[!0-9]*) ;;
        *) echo "run.sh: preview suffix must be a DNS name, not an IP address" >&2; exit 1 ;;
    esac
    case "$label" in
        0x*) echo "run.sh: preview suffix has an invalid final DNS label" >&2; exit 1 ;;
    esac
    case "$domain" in
        "$preview_domain"|*."$preview_domain")
            echo "run.sh: preview namespace must not contain the hub hostname" >&2; exit 1 ;;
    esac
    # A different SAN set needs a new order, even if the old hub certificate is fresh.
    state=$state/previews/$preview_domain
fi
chain=$state/certificates/$domain.crt
key=$state/certificates/$domain.key
retry_after_failure=${ANIMA_RETRY_AFTER_FAILURE_SECONDS:-3600}
renew_every=${ANIMA_RENEW_EVERY_SECONDS:-43200}

lego_cmd() {
    if [ -n "$preview_domain" ]; then
        set -- --domains "*.$preview_domain" "$@"
    fi
    "$lego" --accept-tos --email "$ANIMA_ACME_EMAIL" --server "$server" \
        --dns cloudflare --domains "$domain" --path "$state" "$@"
}

install_current() {
    LEGO_CERT_PATH=$chain LEGO_CERT_KEY_PATH=$key sh "$hook"
}

# The install after each check also installs a renewal whose hook failed.
check_renewal() {
    if ! lego_cmd renew --days 30 --renew-hook "$hook"; then
        echo "run.sh: renewal check failed; trying again in $renew_every seconds" >&2
    fi
    if ! install_current; then
        echo "run.sh: installing the certificate failed; trying again in $renew_every seconds" >&2
    fi
}

if [ -s "$chain" ]; then
    check_renewal
elif ! lego_cmd run --run-hook "$hook"; then
    # Wait before the restart so a failing order does not hit the Let's Encrypt rate limit.
    echo "run.sh: the first certificate order failed; retrying in $retry_after_failure seconds" >&2
    sleep "$retry_after_failure"
    exit 1
fi

while true; do
    sleep "$renew_every"
    check_renewal
done
