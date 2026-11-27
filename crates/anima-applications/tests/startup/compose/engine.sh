
root="${0%/*}"
digest="sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
id="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
case "$5:$6" in
    version:--format) printf '28.0.0\n';;
    info:--format) printf '{"root":"%s","memory":8589934592,"processors":4}\n' "$root";;
    image:pull)
        test "$7 $8" = '-- library/alpine:3.22' || exit 1
        printf 'Digest: %s\n' "$digest"
        printf 'pull\n' >> "$root/pulled"
        if test -f "$root/revive-on-pull"; then : > "$root/source-running"; fi
        if test -f "$root/unmask-on-pull"; then printf 'enabled' > "$root/source-unit-state"; fi
        if test -f "$root/recreate-on-pull"; then : > "$root/source-replaced"; fi;;
    image:inspect)
        test "${10}" = "library/alpine@$digest" || exit 1
        test ! -f "$root/reject-digest" || exit 1
        printf '%s\n' "$digest";;
    container:ls)
        test "${11}" = 'label=com.docker.compose.project=anima-0192f6b07c1e7a3b9d2f4e5a6b7c8d9e' || exit 1
        if test -f "$root/started"; then printf '%s\n' "$id"; fi;;
    container:inspect)
        test "${10}" = "$id" || exit 1
        folder="$root/applications/0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9e/files"
        if test -f "$root/foreign-project"; then folder=/foreign; fi
        health=healthy
        if test -f "$root/unhealthy-docker"; then health=unhealthy; fi
        printf '{"id":"%s","name":"/demo","project":"anima-0192f6b07c1e7a3b9d2f4e5a6b7c8d9e","folder":"%s","service":"demo","running":true,"health":"%s"' "$id" "$folder" "$health"
        case "$8" in
            *final_form*)
                source="${folder%/files}/binds/state"
                if test -f "$root/wrong-docker-bind"; then source=/legacy/state; fi
                config_hash=cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc
                if test -f "$root/stale-docker-config"; then config_hash=${id}; fi
                printf ',"final_form":"%s/final-form.json","config_hash":"%s","image":"library/alpine@%s","mounts":[{"Type":"bind","Source":"%s","Destination":"/persisted","RW":true}]' "$folder" "$config_hash" "$digest" "$source";;
        esac
        printf '}\n';;
    *) exit 1;;
esac
