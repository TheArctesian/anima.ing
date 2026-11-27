#!/bin/sh
root="${0%/legacy-bin/*}"
test "$1" = --remote=false || exit 1
case "$2:$3" in
    info:--format)
        store=/legacy/original
        if test -f "$root/different-store"; then store=/legacy/other; fi
        rootless=true
        if test "$(/usr/bin/id -u)" = 0; then rootless=false; fi
        printf '{"root":"%s","rootless":%s}\n' "$store" "$rootless";;
    container:inspect)
        test "$6" = '--' && test "$7" = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb' || exit 1
        test ! -f "$root/source-removed" || exit 1
        case "$5" in
            *autoRemove*)
                printf '{"id":"%s","name":"systemd-example","autoRemove":true,"unit":"container-example.service"}\n' "$7"
                exit;;
        esac
        restart=no
        if test -f "$root/source-restart"; then restart=always; fi
        running=false
        pid=0
        if test -f "$root/source-running"; then running=true; pid=500; fi
        unit=
        if test -f "$root/with-source-unit"; then
            case "$5" in *systemdUnit*) unit=',"systemdUnit":"container-example.service"';; esac
        fi
        printf '{"id":"%s","state":"exited","pid":%s,"running":%s,"paused":false,"restarting":false,"restart":"%s"%s}\n' "$7" "$pid" "$running" "$restart" "$unit";;
    ps:--all)
        test "$4 $5" = '--no-trunc --format' || exit 1
        case "$6" in *'.Names'*) ;; *) exit 1;; esac
        id=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
        if test -f "$root/source-replaced"; then
            id=dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd
        elif test -f "$root/source-removed"; then
            exit 0
        fi
        printf '{"id":"%s","name":"systemd-example"}\n' "$id";;
    *) exit 1;;
esac
