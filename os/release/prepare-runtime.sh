#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ "$(uname -s)/$(uname -m)" != Linux/x86_64 ]]; then
  echo "AnimaOS runtime payload requires an x86_64 Linux builder." >&2
  exit 1
fi
# A local preview build using the repository's pinned Rust toolchain and lockfile.
# Only explicit runtime sources, binaries and provenance enter the Nix payload.
cargo build --locked --release -p anima-daemon
mkdir -p os/payload
install -m755 target/release/anima-daemon os/payload/anima-daemon
strip os/payload/anima-daemon
git rev-parse HEAD > os/payload/SOURCE-REVISION
sha256sum Cargo.lock > os/payload/CARGO-LOCK-SHA256
sha256sum os/payload/anima-daemon > os/payload/BINARY-SHA256

(cd link && CGO_ENABLED=0 go build -trimpath -ldflags='-s -w' -o ../os/payload/anima-link ./cmd/anima-link)
sha256sum link/go.sum > os/payload/GO-SUM-SHA256
sha256sum os/payload/anima-link > os/payload/LINK-BINARY-SHA256

source_payload=os/payload/source
rm -rf -- "$source_payload"
bash os/release/project-sources.sh "$source_payload"
(cd "$source_payload" && find . -type f -print0 | sort -z | xargs -0 sha256sum) > os/payload/SOURCE-SHA256
