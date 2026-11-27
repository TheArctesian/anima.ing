#!/usr/bin/env bash
set -euo pipefail
out=$1
mode=$2
test_tools=${3:-false}
if [[ -n "$(git status --porcelain --untracked-files=all)" || -e os/payload ]]; then
  echo "OS assembly requires a clean checkout with no existing runtime payload." >&2
  exit 1
fi
if [[ "$mode" != signed && "$mode" != unsigned ]]; then
  echo "Unknown OS assembly mode." >&2
  exit 2
fi
os/release/prepare-runtime.sh
if [[ -n "$(git status --porcelain --untracked-files=all)" ]]; then
  echo "Runtime preparation changed the candidate source." >&2
  exit 1
fi
python3 os/release/generation.py --record-inputs "$out/build-inputs.json"
# Runtime binaries are now in payload; reclaim the disposable runner's Rust build.
rm -rf target
nix --extra-experimental-features 'nix-command flakes' build --no-update-lock-file \
  path:./os#nixosConfigurations.appliance.config.system.build.toplevel --out-link "$out/system" -L
python3 os/release/generation.py "$out/system"
nix --extra-experimental-features 'nix-command flakes' build --no-update-lock-file \
  path:./os#iso --out-link "$out/iso" -L
cp "$out/iso/iso/anima-os-x86_64.iso" "$out/anima-os-x86_64.iso"
receipt_flags=()
if [[ "$mode" == signed ]]; then
  python3 os/updates/package.py "$out/system" "$out/package" --key "$OS_SIGNING_KEY"
else
  receipt_flags+=(--unsigned)
fi
if [[ "$mode" == signed || "$test_tools" == true ]]; then
  bash os/release/private-access.sh "$out/package"
  receipt_flags+=(--test-tools)
fi
python3 os/release/receipt.py "$out" "${receipt_flags[@]}"
