set -euo pipefail
git_fixed() {
    /bin/git -c core.hooksPath=/dev/null -c credential.helper= \
        -c protocol.allow=never -c protocol.https.allow=always \
        -c http.followRedirects=false -c submodule.recurse=false \
        -C /home/agent/workspace "$@"
}
git_fixed init --quiet --initial-branch=source
if [[ "${3:-public}" == private ]]; then
    # Only fetch inherits the credential pipe. Git does not save command-scoped helpers.
    git_fixed -c credential.useHttpPath=true \
        -c 'credential.helper=!f() { if test "$1" = get; then /bin/cat <&3; fi; }; f' \
        fetch --quiet --depth=1 --no-tags --no-recurse-submodules -- "$1" "$2" 3<&0 </dev/null
else
    git_fixed fetch --quiet --depth=1 --no-tags --no-recurse-submodules -- "$1" "$2"
fi
test "$(git_fixed rev-parse --verify 'FETCH_HEAD^{commit}')" = "$2"
git_fixed checkout --quiet --detach --force FETCH_HEAD
if git_fixed ls-files --stage | /bin/grep '^160000 ' > /dev/null; then
    exit 3
fi
git_fixed rev-parse --verify HEAD
