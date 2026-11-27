#!/usr/bin/env bash
# Package the offline private-access fixture from the detached source checkout.
set -euo pipefail
out=$1
mkdir -p "$out"
CGO_ENABLED=0 go -C link test -c -o "$out/private-access.test" ./devicelink
export ANIMA_OS_WORKSPACE="$PWD"
python3 - "$out" <<'PY'
import json, subprocess, sys
from pathlib import Path

out = Path(sys.argv[1])
packages = {'freerdp': 'freerdp', 'xorg': 'xorg.xorgserver', 'openssl': 'openssl.bin',
            'imagemagick': 'imagemagick', 'tesseract': 'tesseract'}
paths = {}
for name, attr in packages.items():
    expr = ('let f = builtins.getFlake ("path:" + builtins.getEnv "ANIMA_OS_WORKSPACE" + "/os"); '
            'p = import f.inputs.nixpkgs { system = "x86_64-linux"; }; in p.' + attr)
    paths[name] = subprocess.check_output(
        ['nix', '--extra-experimental-features', 'nix-command flakes', 'build',
         '--impure', '--expr', expr, '--no-link', '--print-out-paths'], text=True).strip()
(out / 'test-tools.json').write_text(json.dumps(paths))
requisites = subprocess.check_output(['nix-store', '-qR', *paths.values()], text=True).splitlines()
with (out / 'test-tools.nar').open('wb') as stream:
    subprocess.run(['nix-store', '--export', *sorted(set(requisites))], stdout=stream, check=True)
PY
