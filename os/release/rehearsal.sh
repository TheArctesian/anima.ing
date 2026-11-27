#!/usr/bin/env bash
# Build a detached legacy baseline and candidate using a disposable verification key.
set -euo pipefail
baseline=$1
out=$2
scratch=$3
recipe="$PWD/os/release"
public="$out/fixture-public.pem"
private="$scratch/fixture-private.pem"
(
  umask 077
  openssl genpkey -quiet -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "$private"
  openssl pkey -in "$private" -pubout -out "$public"
)

build_images() (
  directory=$1
  updater=$2
  output=$3
  mkdir -p "$output"
  if [[ -n "$(git status --porcelain --untracked-files=all)" || -e "$directory/payload" ]]; then
    echo "Rehearsal assembly requires a detached checkout without existing payloads." >&2
    exit 1
  fi
  if [[ "$directory" == os ]]; then
    bash os/release/prepare-runtime.sh
  else
    bash "$directory/prepare-runtime.sh"
  fi
  if [[ -n "$(git status --porcelain --untracked-files=all)" ]]; then
    echo "Rehearsal runtime preparation changed the committed inputs." >&2
    exit 1
  fi
  python3 "$recipe/generation.py" --record-inputs "$output/build-inputs.json" \
    --os-directory "$directory" --fixture-public-key "$public"
  rm -rf target
  flags=(--extra-experimental-features 'nix-command flakes' build --impure --no-update-lock-file
    --file "$recipe/rehearsal-images.nix" --argstr workspace "$PWD" --argstr osDirectory "$directory"
    --argstr updaterFile "$updater" --argstr fixtureKey "$public")
  nix "${flags[@]}" system --out-link "$output/system" -L
  cmp "$public" "$output/system/anima-rehearsal-public.pem"
  if [[ "$directory" == os ]]; then python3 "$recipe/generation.py" "$output/system"; fi
  nix "${flags[@]}" iso --out-link "$output/iso" -L
  cp "$output/iso/iso/anima-os-x86_64.iso" "$output/anima-os-x86_64.iso"
)

git clone --quiet --shared --no-checkout -- "$PWD" "$scratch/baseline"
git -C "$scratch/baseline" -c core.hooksPath=/dev/null checkout --quiet --detach "$baseline"
(
  cd "$scratch/baseline"
  build_images experiments/anima-os update-package.nix "$out/baseline"
  python3 "$recipe/receipt.py" --unsigned "$out/baseline"
)
build_images os packages/update-package.nix "$out/candidate"
python3 os/updates/package.py "$out/candidate/system" "$out/candidate/package" --key "$private"
rm -f "$private"
bash "$recipe/private-access.sh" "$out/candidate/package"
python3 "$recipe/receipt.py" --unsigned --test-tools "$out/candidate"
