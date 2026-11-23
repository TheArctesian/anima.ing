#!/bin/sh
# Makes the iOS archive for one version of the checked-out commit (M5-4; RLS-010, RLS-011); it uploads nothing (R-203).
#
#   ios-archive.sh [version]          with no version: the version of the checked-out commit (R-183, R-201)
#   ios-archive.sh --plan [version]   prints what a run with this environment makes, and builds nothing
#
# Environment:
#   IOS_SIGN_IDENTITY   Unset or empty: an archive with no signature and no IPA. Set: the name of the Apple
#                       Distribution identity, as `security find-identity` prints it, in a keychain of the search list.
#   IOS_TESTFLIGHT_BUILD  Optional TestFlight version number. Keeps the signed archive for direct Xcode upload.
#   IOS_TESTFLIGHT_EXPORT_EXEMPT  Set to true for an internal build with a reviewed export exemption.
#   IOS_PROFILE         Needed with IOS_SIGN_IDENTITY: the path of the App Store provisioning profile of ing.anima.mobile.
#
# The script reads no certificate file and no password, and prints no environment value but the identity name and profile UUID.
# A signature is by hand, so Xcode makes no certificate or profile and changes nothing in the developer account.
# Settings go into gen/apple/project.yml: Tauri gives xcodebuild no caller setting and ignores XCODE_XCCONFIG_FILE.
# Tauri's own export is not used: with --no-sign it looks for Anima.app (tauri-cli 2.12.4), signed it lets Xcode change profiles.
#
# It prints, as its last lines:
#   version=<version>
#   build=<build number>
#   signed=yes|no
#   archive=<path of the .xcarchive>
#   ipa=<path of the .ipa>           only with a signature
#
# Needs: Xcode with the iOS platform, xcodegen, bun with the packages installed, Go and the Rust target aarch64-apple-ios.
set -eu

refuse() { echo "ios-archive: $1" >&2; exit 1; }

PLAN_ONLY=false
if [ "${1:-}" = --plan ]; then PLAN_ONLY=true; shift; fi
[ $# -le 1 ] || refuse "give one version, or none for the version of the commit"
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../../.." && pwd)
APP_ID=ing.anima.mobile
VERSION=${1:-$(cd "$ROOT" && sh deploy/bin/commit-version)}
echo "$VERSION" | grep -Eq '^(0|[1-9][0-9]{0,8})\.(0|[1-9][0-9]{0,8})\.(0|[1-9][0-9]{0,8})$' || refuse "the version is not like 0.1.0"
BUILD=$(sh "$ROOT/deploy/bin/phone-build-number" "$VERSION") || refuse "the version has no build number"
# TestFlight builds take the workflow's sequence; other archives keep the commit-derived number.
if [ -n "${IOS_TESTFLIGHT_BUILD:-}" ]; then
    echo "$IOS_TESTFLIGHT_BUILD" | grep -Eq '^[1-9][0-9]{0,3}\.(0|[1-9][0-9]?)\.(0|[1-9][0-9]?)$' || refuse "invalid TestFlight build number"
    BUILD=$IOS_TESTFLIGHT_BUILD
fi
case "${IOS_TESTFLIGHT_EXPORT_EXEMPT:-}" in
    '') ;;
    true) [ -n "${IOS_TESTFLIGHT_BUILD:-}" ] || refuse "the export exemption requires a TestFlight build number" ;;
    *) refuse "IOS_TESTFLIGHT_EXPORT_EXEMPT must be true or unset" ;;
esac
IDENTITY=${IOS_SIGN_IDENTITY:-}
PROFILE=${IOS_PROFILE:-}
# The identity goes into a settings file of Xcode and into a property list: one plain line.
case "$IDENTITY" in *\"* | *\\* | *'
'* | *\$* | *\<* | *\&*) refuse "IOS_SIGN_IDENTITY holds a character that a name of an identity does not have" ;; esac
if [ -n "$IDENTITY" ]; then
    SIGNED=yes
    [ -n "$PROFILE" ] || refuse "IOS_SIGN_IDENTITY is set, so IOS_PROFILE must name the provisioning profile"
else
    SIGNED=no
    [ -z "$PROFILE" ] || refuse "IOS_PROFILE is set, so IOS_SIGN_IDENTITY must name the identity"
fi
PROJECT=$HERE/gen/apple
ARCHIVE=$PROJECT/build/anima-app_iOS.xcarchive
OUT=$PROJECT/build/arm64
IPA=$OUT/Anima.ing.ipa
if $PLAN_ONLY; then
    printf 'version=%s\nbuild=%s\nsigned=%s\n' "$VERSION" "$BUILD" "$SIGNED"
    exit 0
fi

ROOM=$(mktemp -d)
trap 'rm -rf "$ROOM"' EXIT
plist() { /usr/libexec/PlistBuddy -c "Print :$1" "$2"; }

if [ "$SIGNED" = yes ]; then
    [ -f "$PROFILE" ] || refuse "IOS_PROFILE is not a file"
    security cms -D -i "$PROFILE" > "$ROOM/profile.plist" 2> /dev/null || refuse "IOS_PROFILE is not a provisioning profile"
    UUID=$(plist UUID "$ROOM/profile.plist")
    TEAM=$(plist TeamIdentifier:0 "$ROOM/profile.plist")
    echo "$UUID" | grep -Eq '^[0-9A-Fa-f-]{36}$' || refuse "the profile has no UUID"
    echo "$TEAM" | grep -Eq '^[A-Z0-9]{10}$' || refuse "the profile has no team"
    [ "$(plist Entitlements:application-identifier "$ROOM/profile.plist")" = "$TEAM.$APP_ID" ] || refuse "the profile is not for $APP_ID"
    [ "$(plist Entitlements:get-task-allow "$ROOM/profile.plist")" = false ] || refuse "the profile is a development profile, not one for App Store Connect"
    security find-identity -v -p codesigning | grep -Fq "\"$IDENTITY\"" || refuse "no keychain of the search list holds the identity that IOS_SIGN_IDENTITY names"
    for folder in "$HOME/Library/MobileDevice/Provisioning Profiles" "$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles"; do
        mkdir -p "$folder"
        cp "$PROFILE" "$folder/$UUID.mobileprovision"
    done
    echo "ios-archive: a signature by hand with the identity $IDENTITY and the profile $UUID"
fi

signature_settings() {
    if [ "$SIGNED" = yes ]; then
        echo "      CODE_SIGN_STYLE: Manual"
        echo "      CODE_SIGN_IDENTITY: \"$IDENTITY\""
        echo "      DEVELOPMENT_TEAM: $TEAM"
        echo "      PROVISIONING_PROFILE_SPECIFIER: $UUID"
    else
        echo "      CODE_SIGNING_ALLOWED: \"NO\""
        echo "      CODE_SIGNING_REQUIRED: \"NO\""
        echo "      CODE_SIGN_IDENTITY: \"\""
    fi
}

sh "$ROOT/link/build-phone.sh" aarch64-apple-ios
cd "$HERE/.."
# Xcode runs Tauri's Cargo build in a child process, which can otherwise find a system Rust.
RUST_CARGO=$(rustup which cargo) || refuse "install the Rust toolchain of the repository with rustup"
PATH="$(dirname "$RUST_CARGO"):$PATH"
export PATH
export CI=true
# A new project each time, so that no setting of an earlier run stays in it.
rm -rf "$PROJECT/anima-app.xcodeproj" "$PROJECT/project.yml" "$ARCHIVE" "$IPA"
bunx tauri ios init --ci
grep -q '^      PRODUCT_BUNDLE_IDENTIFIER: ' "$PROJECT/project.yml" || refuse "project.yml of Tauri has no group of settings where the script looks for it"
cp -R "$HERE/ios/AnimaAppIcon.appiconset" "$PROJECT/Assets.xcassets/"
cp "$HERE/ios/privacy-manifest.plist" "$PROJECT/anima-app_iOS/PrivacyInfo.xcprivacy"
{
    signature_settings
    echo '      ASSETCATALOG_COMPILER_APPICON_NAME: AnimaAppIcon'
    echo '      TARGETED_DEVICE_FAMILY: "1,2"'
} > "$ROOM/settings"
# The profile's team replaces Tauri's; the settings go after the identifier line of the group `app`.
if [ "$SIGNED" = yes ]; then sed -i '' -e '/^      DEVELOPMENT_TEAM: /d' "$PROJECT/project.yml"; fi
sed -i '' -e "/^      PRODUCT_BUNDLE_IDENTIFIER: /r $ROOM/settings" "$PROJECT/project.yml"
(cd "$PROJECT" && xcodegen generate --spec project.yml --quiet)
if [ "${IOS_TESTFLIGHT_EXPORT_EXEMPT:-}" = true ]; then
    plutil -insert ITSAppUsesNonExemptEncryption -bool NO "$PROJECT/anima-app_iOS/Info.plist"
fi
CONFIG="{\"version\":\"$VERSION\",\"bundle\":{\"iOS\":{\"bundleVersion\":\"$BUILD\"}}}"
bunx tauri ios build --target aarch64 --ci --archive-only --config "$CONFIG"

APP=$ARCHIVE/Products/Applications/Anima.ing.app
[ -d "$APP" ] || refuse "the build made no archive at $ARCHIVE"
[ -f "$APP/PrivacyInfo.xcprivacy" ] || refuse "the archive does not contain the app privacy manifest"
[ "$(plist CFBundleIdentifier "$APP/Info.plist")" = "$APP_ID" ] || refuse "the application of the archive is not $APP_ID"
[ "$(plist CFBundleShortVersionString "$APP/Info.plist")" = "$VERSION" ] || refuse "the archive does not have the version $VERSION"
[ "$(plist CFBundleVersion "$APP/Info.plist")" = "$BUILD" ] || refuse "the archive does not have the build number $BUILD"
if [ "${IOS_TESTFLIGHT_EXPORT_EXEMPT:-}" = true ]; then
    [ "$(plist ITSAppUsesNonExemptEncryption "$APP/Info.plist")" = false ] ||
        refuse "the archive does not contain the reviewed TestFlight export exemption"
fi
for family in CFBundleIcons 'CFBundleIcons~ipad'; do
    [ "$(plist "$family:CFBundlePrimaryIcon:CFBundleIconName" "$APP/Info.plist")" = AnimaAppIcon ] ||
        refuse "the archive does not have the Anima icon for $family"
done

if [ "$SIGNED" = no ]; then
    if codesign --display "$APP" > /dev/null 2>&1; then refuse "the application of the archive has a signature, and this run asked for none"; fi
    printf 'version=%s\nbuild=%s\nsigned=no\narchive=%s\n' "$VERSION" "$BUILD" "$ARCHIVE"
    exit 0
fi
codesign --verify --deep --strict "$APP" || refuse "the signature of the application in the archive is not valid"
if [ -n "${IOS_TESTFLIGHT_BUILD:-}" ]; then
    printf 'version=%s\nbuild=%s\nsigned=yes\narchive=%s\n' "$VERSION" "$BUILD" "$ARCHIVE"
    exit 0
fi
cat > "$ROOM/export.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>method</key><string>app-store-connect</string>
    <key>destination</key><string>export</string>
    <key>signingStyle</key><string>manual</string>
    <key>teamID</key><string>$TEAM</string>
    <key>signingCertificate</key><string>$IDENTITY</string>
    <key>provisioningProfiles</key><dict><key>$APP_ID</key><string>$UUID</string></dict>
    <key>manageAppVersionAndBuildNumber</key><false/>
    <key>uploadSymbols</key><true/>
</dict>
</plist>
EOF
mkdir -p "$OUT"
xcodebuild -exportArchive -archivePath "$ARCHIVE" -exportOptionsPlist "$ROOM/export.plist" -exportPath "$OUT"
[ -f "$IPA" ] || refuse "the export made no IPA at $IPA"
printf 'version=%s\nbuild=%s\nsigned=yes\narchive=%s\nipa=%s\n' "$VERSION" "$BUILD" "$ARCHIVE" "$IPA"
