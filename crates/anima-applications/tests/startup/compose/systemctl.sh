#!/bin/sh
root="${0%/legacy-bin/*}"
uid=$(/usr/bin/id -u)
scope=--system
if test "$uid" != 0; then
    scope=--user
    test "$XDG_RUNTIME_DIR" = "/run/user/$uid" || exit 1
fi
test "$*" = "$scope --no-pager --all --property=Id,LoadState,ActiveState,UnitFileState show -- container-example.service" || exit 1
IFS= read -r state < "$root/source-unit-state"
test -n "$state" || exit 1
load=loaded
active=inactive
case "$state" in
    masked|masked-runtime) load=masked;;
    masked-active) load=masked; state=masked; active=active;;
esac
printf 'Id=container-example.service\nLoadState=%s\nActiveState=%s\nUnitFileState=%s\n' "$load" "$active" "$state"
