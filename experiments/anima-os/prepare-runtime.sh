#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
# A local preview build using the repository's pinned Rust toolchain and lockfile.
# Only explicit runtime sources, binaries and provenance enter the Nix payload.
cargo build --locked --release -p anima-daemon
mkdir -p experiments/anima-os/payload
install -m755 target/release/anima-daemon experiments/anima-os/payload/anima-daemon
strip experiments/anima-os/payload/anima-daemon
git rev-parse HEAD > experiments/anima-os/payload/SOURCE-REVISION
sha256sum Cargo.lock > experiments/anima-os/payload/CARGO-LOCK-SHA256
sha256sum experiments/anima-os/payload/anima-daemon > experiments/anima-os/payload/BINARY-SHA256

(cd link && CGO_ENABLED=0 go build -trimpath -ldflags='-s -w' -o ../experiments/anima-os/payload/anima-link ./cmd/anima-link)
sha256sum link/go.sum > experiments/anima-os/payload/GO-SUM-SHA256
sha256sum experiments/anima-os/payload/anima-link > experiments/anima-os/payload/LINK-BINARY-SHA256

source_payload=experiments/anima-os/payload/source
rm -rf -- "$source_payload"
mkdir -p "$source_payload/crates"
cp Cargo.toml Cargo.lock "$source_payload/"
for crate in anima-osd anima-os-session anima-os-update anima-os-install; do
  mkdir -p "$source_payload/crates/$crate"
  cp "crates/$crate/Cargo.toml" "$source_payload/crates/$crate/"
  cp -R "crates/$crate/src" "$source_payload/crates/$crate/"
  if [[ -d "crates/$crate/tests" ]]; then
    cp -R "crates/$crate/tests" "$source_payload/crates/$crate/"
  fi
done
(cd "$source_payload" && find . -type f -print0 | sort -z | xargs -0 sha256sum) > experiments/anima-os/payload/SOURCE-SHA256
