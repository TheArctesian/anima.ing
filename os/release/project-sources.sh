#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
destination="${1:?Pass the projected source directory}"
mkdir -p "$destination/crates"
cp "$root"/{Cargo.toml,Cargo.lock,rust-toolchain.toml} "$destination/"
crates=(
  anima-applications anima-osd anima-os-session anima-os-update anima-os-install
  anima-os-maintenance anima-os-execution anima-os-storage
  agent-threads anima-applications-wire compose-rules daemon-core oci-artifact
  power redaction remote-login
)
for crate in "${crates[@]}"; do
  source="$root/crates/$crate"
  target="$destination/crates/$crate"
  mkdir -p "$target"
  cp "$source/Cargo.toml" "$target/"
  cp -R "$source/src" "$target/"
  if [[ -d "$source/tests" ]]; then
    cp -R "$source/tests" "$target/"
  fi
done
