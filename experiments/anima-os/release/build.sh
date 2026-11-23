#!/usr/bin/env bash
set -euo pipefail
out=$(realpath "$1")
mkdir -p "$out"
experiments/anima-os/prepare-runtime.sh
# Runtime binaries are now in payload; reclaim the disposable runner's Rust build.
rm -rf target
nix --extra-experimental-features 'nix-command flakes' build path:./experiments/anima-os#iso --out-link "$out/iso" -L
nix --extra-experimental-features 'nix-command flakes' build path:./experiments/anima-os#nixosConfigurations.appliance.config.system.build.toplevel --out-link "$out/system" -L
cp "$out/iso/iso/anima-os-x86_64.iso" "$out/anima-os-x86_64.iso"
python3 experiments/anima-os/updates/package.py "$out/system" "$out/package" --key "$OS_SIGNING_KEY"
CGO_ENABLED=0 go -C link test -c -o "$out/private-access.test" ./devicelink
python3 experiments/anima-os/release/receipt.py "$out"
export ANIMA_OS_WORKSPACE="$PWD"
python3 - "$out" <<'PY'
import json, os, subprocess, sys
from pathlib import Path
out = Path(sys.argv[1])
paths = {}
for name, attr in [('freerdp', 'freerdp'), ('xorg', 'xorg.xorgserver'), ('openssl', 'openssl.bin')]:
    expr = 'let f = builtins.getFlake ("path:" + builtins.getEnv "ANIMA_OS_WORKSPACE" + "/experiments/anima-os"); p = import f.inputs.nixpkgs { system = "x86_64-linux"; }; in p.' + attr
    paths[name] = subprocess.check_output(['nix', '--extra-experimental-features', 'nix-command flakes', 'build', '--impure', '--expr', expr, '--no-link', '--print-out-paths'], text=True).strip()
(out / 'package/test-tools.json').write_text(json.dumps(paths))
requisites = subprocess.check_output(['nix-store', '-qR', *paths.values()], text=True).splitlines()
with (out / 'package/test-tools.nar').open('wb') as stream:
    subprocess.run(['nix-store', '--export', *sorted(set(requisites))], stdout=stream, check=True)
PY
mv "$out/private-access.test" "$out/package/"
