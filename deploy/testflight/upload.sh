#!/bin/bash
set -euo pipefail
umask 077
: "${ASC_API_PRIVATE_KEY:?Set the App Store Connect key in the release environment}"
: "${ASC_API_KEY_ID:?Set the App Store Connect key ID in the release environment}"
: "${ASC_API_ISSUER_ID:?Set the App Store Connect issuer ID in the release environment}"

case "$GITHUB_REF_NAME" in
    dev) internal=true ;;
    main) internal=false ;;
    *) exit 1 ;;
esac

room=$(mktemp -d "$RUNNER_TEMP/testflight-upload.XXXXXX")
trap 'rm -rf "$room"' EXIT
key="$room/AuthKey_$ASC_API_KEY_ID.p8"
printf '%s\n' "$ASC_API_PRIVATE_KEY" > "$key"
security cms -D -i "$IOS_PROFILE" > "$room/profile.plist"
team=$(/usr/libexec/PlistBuddy -c 'Print :TeamIdentifier:0' "$room/profile.plist")
profile=$(/usr/libexec/PlistBuddy -c 'Print :UUID' "$room/profile.plist")
cat > "$room/export.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>method</key><string>app-store-connect</string>
<key>destination</key><string>upload</string>
<key>signingStyle</key><string>manual</string>
<key>teamID</key><string>$team</string>
<key>signingCertificate</key><string>Apple Distribution</string>
<key>provisioningProfiles</key><dict><key>ing.anima.mobile</key><string>$profile</string></dict>
<key>manageAppVersionAndBuildNumber</key><false/>
<key>testFlightInternalTestingOnly</key><$internal/>
</dict></plist>
PLIST

present=0
node deploy/testflight/groups.mjs present || present=$?
case "$present" in
    0) echo "This build is already in App Store Connect." ;;
    3) xcodebuild -exportArchive -archivePath "$TESTFLIGHT_ARCHIVE" \
           -exportOptionsPlist "$room/export.plist" -exportPath "$room/export" \
           -authenticationKeyPath "$key" -authenticationKeyID "$ASC_API_KEY_ID" \
           -authenticationKeyIssuerID "$ASC_API_ISSUER_ID" ;;
    *) exit "$present" ;;
esac
node deploy/testflight/groups.mjs distribute
