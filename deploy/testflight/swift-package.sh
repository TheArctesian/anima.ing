#!/bin/sh
# Keep SwiftPM's Git fetch off the network during the archive. Tauri's iOS API uses SwiftRs 1.0.9.
set -eu

swift_temp=${1:?Give a temporary directory for the Swift package}
source_url=https://github.com/Brendonovich/swift-rs
swift_tag=1.0.9
swift_revision=207e4bfe5d36fdd0cc08f28d1849ff94ac7e24b8
bundle="$(dirname "$0")/swift-rs-$swift_tag.bundle"

swift_work=$(mktemp -d "$swift_temp/anima-swift-rs.XXXXXX")
swift_mirror="$swift_work/swift-rs.git"
git clone --quiet --bare "$bundle" "$swift_mirror"
actual_revision=$(git -C "$swift_mirror" rev-parse "refs/tags/$swift_tag^{commit}")
if [ "$actual_revision" != "$swift_revision" ]; then
  echo "::error::The SwiftRs bundle does not contain the pinned upstream release."
  exit 1
fi

{
  printf 'GIT_CONFIG_COUNT=2\n'
  printf 'GIT_CONFIG_KEY_0=url.file://%s.insteadOf\n' "$swift_mirror"
  printf 'GIT_CONFIG_VALUE_0=%s.git\n' "$source_url"
  printf 'GIT_CONFIG_KEY_1=url.file://%s.insteadOf\n' "$swift_mirror"
  printf 'GIT_CONFIG_VALUE_1=%s\n' "$source_url"
}
