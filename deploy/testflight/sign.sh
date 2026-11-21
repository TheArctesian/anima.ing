#!/bin/bash
# The keychain and profile are on the disposable runner. The upload key is supplied separately.
set -euo pipefail
umask 077
: "${IOS_DISTRIBUTION_P12_BASE64:?Set the iOS certificate in the release environment}"
: "${IOS_DISTRIBUTION_P12_PASSWORD:?Set the iOS certificate password in the release environment}"
: "${IOS_PROVISIONING_PROFILE_BASE64:?Set the iOS provisioning profile in the release environment}"

keychain="$RUNNER_TEMP/testflight.keychain-db"
password=$(openssl rand -hex 24)
security create-keychain -p "$password" "$keychain"
security set-keychain-settings -lut 21600 "$keychain"
security unlock-keychain -p "$password" "$keychain"
certificate=$(mktemp "$RUNNER_TEMP/distribution.XXXXXX")
trap 'rm -f "$certificate"' EXIT
printf '%s' "$IOS_DISTRIBUTION_P12_BASE64" | base64 --decode > "$certificate"
security import "$certificate" -f pkcs12 -P "$IOS_DISTRIBUTION_P12_PASSWORD" \
    -k "$keychain" -T /usr/bin/codesign >/dev/null
security set-key-partition-list -S apple-tool:,apple: -s -k "$password" "$keychain" >/dev/null
security list-keychains -d user -s "$keychain" login.keychain-db
identity=$(security find-identity -v -p codesigning "$keychain" | sed -n 's/.*"\(Apple Distribution: [^"]*\)".*/\1/p')
test -n "$identity"
profile="$RUNNER_TEMP/testflight.mobileprovision"
printf '%s' "$IOS_PROVISIONING_PROFILE_BASE64" | base64 --decode > "$profile"

version=$(sh deploy/bin/commit-version origin/main)
if [[ "$GITHUB_REF_NAME" == main ]]; then version=$(sh deploy/bin/commit-version HEAD); fi
number=$GITHUB_RUN_NUMBER
build="$((900 + number / 10000)).$((number / 100 % 100)).$((number % 100))"

# The archive script verifies the signed bundle identifier, version and build number.
export IOS_SIGN_IDENTITY="$identity" IOS_PROFILE="$profile" IOS_TESTFLIGHT_BUILD="$build"
sh apps/desktop/src-tauri/ios-archive.sh "$version" | tee "$RUNNER_TEMP/testflight-archive.log"
archive=$(sed -n 's/^archive=//p' "$RUNNER_TEMP/testflight-archive.log")
test -d "$archive"
{
    printf 'TESTFLIGHT_VERSION=%s\nTESTFLIGHT_BUILD=%s\nTESTFLIGHT_ARCHIVE=%s\n' "$version" "$build" "$archive"
    printf 'IOS_PROFILE=%s\n' "$profile"
} >> "$GITHUB_ENV"
