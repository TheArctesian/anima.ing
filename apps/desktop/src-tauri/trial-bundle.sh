#!/bin/sh
# Makes a trial bundle with the hub programs and the daemon (HMD-052, R-206) by the release builder's bundle steps.
# A trial that passes does not prove a release job passes; a trial bundle does not update itself (UPD-001, DEF-001).
# By hand it is never a release: a release starts only from the Builds view or the release workflow (BLD-001, UPD-007).
#
#   trial-bundle.sh [version]    with no version: the version of the checked-out commit (R-183)
#   trial-bundle.sh --marks      prints the marks a build with this environment gets, and builds nothing
#
# Environment:
#   ANIMA_RELEASE_BUILD   1 (release workflow only): the application has the updater; on macOS it needs SIGN_IDENTITY.
#   SIGN_IDENTITY         macOS only. Unset or empty: an ad hoc signature; else the Developer ID identity (R-179).
#
# Needs what the gates need, the tools of database-tree/build.sh and, on macOS, both Rust targets.
set -eu

MARKS_ONLY=false
if [ "${1:-}" = --marks ]; then MARKS_ONLY=true; shift; fi
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../../.." && pwd)
VERSION=${1:-$(cd "$ROOT" && sh deploy/bin/commit-version)}
IDENTITY=${SIGN_IDENTITY:-}
# The identity goes into a JSON text: no quote and no backslash.
case "$IDENTITY" in *\"* | *\\*) echo "trial-bundle: SIGN_IDENTITY holds a quote or a backslash" >&2; exit 1 ;; esac
echo "$VERSION" | grep -Eq '^(0|[1-9][0-9]{0,8})\.(0|[1-9][0-9]{0,8})\.(0|[1-9][0-9]{0,8})$' || { echo "trial-bundle: the version is not like 0.1.0" >&2; exit 1; }
TARGET_FOLDER=${CARGO_TARGET_DIR:-$ROOT/target}
STAGE=$HERE/binaries
RUST_PROGRAMS="anima anima-local-hub anima-hub anima-database anima-gateway anima-daemon"
EXTERNAL='"binaries/anima","binaries/anima-local-hub","binaries/anima-hub","binaries/anima-database","binaries/anima-gateway","binaries/anima-daemon","binaries/anima-link"'
CARRIED="\"externalBin\":[$EXTERNAL],\"resources\":{\"database\":\"database\"}"
export CI=true
# A trial must not check the feed: the published version can be lower, and it would install it (DEF-001).
# The shell checks only whether its mark is set, so the marks are unset, never emptied.
if [ "${ANIMA_RELEASE_BUILD:-}" = 1 ]; then
    if [ "$(uname -s)" = Darwin ] && [ -z "$IDENTITY" ]; then
        echo "trial-bundle: ANIMA_RELEASE_BUILD needs SIGN_IDENTITY on macOS; a release has no ad hoc signature" >&2
        exit 1
    fi
    unset ANIMA_LOCAL_BUILD VITE_ANIMA_LOCAL_BUILD
else
    export ANIMA_LOCAL_BUILD=1
    export VITE_ANIMA_LOCAL_BUILD=1
fi
if [ "$MARKS_ONLY" = true ]; then
    echo "ANIMA_LOCAL_BUILD=${ANIMA_LOCAL_BUILD-unset} VITE_ANIMA_LOCAL_BUILD=${VITE_ANIMA_LOCAL_BUILD-unset}"
    exit 0
fi

cargo_build() {
    ( cd "$ROOT" && ANIMA_VERSION=$VERSION cargo build --release --locked \
        --package anima-cli --bin anima \
        --package anima-local-hub --bin anima-local-hub \
        --package anima-hub --bin anima-hub \
        --package anima-database --bin anima-database \
        --package anima-gateway --bin anima-gateway \
        --package anima-daemon --bin anima-daemon "$@" )
}

tauri() {
    ( cd "$ROOT" && bun run --cwd apps/desktop tauri "$@" )
}

case "$TARGET_FOLDER" in /*) ;; *) echo "trial-bundle: CARGO_TARGET_DIR is not an absolute path" >&2; exit 1 ;; esac
rm -rf "$STAGE" "$TARGET_FOLDER/database-tree" \
    "$TARGET_FOLDER/universal-apple-darwin/release/bundle" "$TARGET_FOLDER/release/bundle"
mkdir -p "$STAGE"

case "$(uname -s)" in
Darwin)
    SIGN_IDENTITY="$IDENTITY" /bin/sh "$HERE/database-tree/build.sh" darwin-universal "$TARGET_FOLDER/database-tree" "$HERE/database"
    /bin/sh "$ROOT/link/build.sh"
    cargo_build --target aarch64-apple-darwin --target x86_64-apple-darwin
    for name in $RUST_PROGRAMS; do
        for triple in aarch64-apple-darwin x86_64-apple-darwin; do
            cp "$TARGET_FOLDER/$triple/release/$name" "$STAGE/$name-$triple"
        done
    done
    cp "$ROOT/link/bin/anima-link-darwin-arm64" "$STAGE/anima-link-aarch64-apple-darwin"
    cp "$ROOT/link/bin/anima-link-darwin-amd64" "$STAGE/anima-link-x86_64-apple-darwin"
    for name in $RUST_PROGRAMS anima-link; do
        lipo -create -output "$STAGE/$name-universal-apple-darwin" \
            "$STAGE/$name-aarch64-apple-darwin" "$STAGE/$name-x86_64-apple-darwin"
    done
    MAC="\"macOS\":{\"signingIdentity\":\"${IDENTITY:--}\"}"
    tauri build --no-bundle --target universal-apple-darwin \
        --config "{\"version\":\"$VERSION\",\"bundle\":{$CARRIED}}"
    tauri bundle --target universal-apple-darwin --bundles app,dmg \
        --config "{\"version\":\"$VERSION\",\"bundle\":{\"createUpdaterArtifacts\":false,$MAC,$CARRIED}}"
    echo "trial-bundle: $TARGET_FOLDER/universal-apple-darwin/release/bundle/macos and .../dmg"
    ;;
Linux)
    triple=$(rustc -vV | sed -n 's/^host: //p')
    case "$triple" in x86_64-*) go_arch=amd64 ;; aarch64-*) go_arch=arm64 ;; *) echo "trial-bundle: no build for $triple" >&2; exit 1 ;; esac
    /bin/sh "$HERE/database-tree/build.sh" linux "$TARGET_FOLDER/database-tree" "$HERE/database"
    /bin/sh "$ROOT/link/build.sh"
    cargo_build
    for name in $RUST_PROGRAMS; do cp "$TARGET_FOLDER/release/$name" "$STAGE/$name-$triple"; done
    cp "$ROOT/link/bin/anima-link-linux-$go_arch" "$STAGE/anima-link-$triple"
    tauri build --no-bundle --config "{\"version\":\"$VERSION\",\"bundle\":{$CARRIED}}"
    APPIMAGE_EXTRACT_AND_RUN=1 tauri bundle --bundles appimage --no-sign \
        --config "{\"version\":\"$VERSION\",\"bundle\":{\"createUpdaterArtifacts\":false,$CARRIED}}"
    echo "trial-bundle: $TARGET_FOLDER/release/bundle/appimage"
    ;;
*)
    echo "trial-bundle: no bundle for this system" >&2
    exit 1
    ;;
esac
