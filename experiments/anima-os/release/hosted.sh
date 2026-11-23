#!/usr/bin/env bash
# Provision only an ephemeral GitHub-hosted Ubuntu runner.
set -euo pipefail
[[ ${GITHUB_ACTIONS:-} == true && ${RUNNER_ENVIRONMENT:-} == github-hosted ]]
sudo rm -rf /usr/local/lib/android /opt/ghc /usr/share/dotnet /opt/hostedtoolcache/CodeQL /usr/local/share/boost
sudo docker system prune -af
sudo apt-get update -q
sudo apt-get install -y --no-install-recommends qemu-system-x86 qemu-utils ovmf tesseract-ocr imagemagick e2fsprogs libdbus-1-dev pkg-config python3-venv
sudo mkdir -p /usr/share/edk2/x64
sudo ln -sf /usr/share/OVMF/OVMF_CODE_4M.fd /usr/share/edk2/x64/OVMF_CODE.4m.fd
sudo ln -sf /usr/share/OVMF/OVMF_VARS_4M.fd /usr/share/edk2/x64/OVMF_VARS.4m.fd
sudo chmod a+rw /dev/kvm
# Ubuntu ships ImageMagick 6; the harness uses the v7 entry point.
if ! command -v magick >/dev/null; then
  printf '#!/bin/sh\ncommand="$1"; shift; exec "$command" "$@"\n' | sudo tee /usr/local/bin/magick >/dev/null
  sudo chmod +x /usr/local/bin/magick
fi
python3 - <<'PY'
import os, shutil
fd = os.open('/dev/kvm', os.O_RDWR); os.close(fd)
free = shutil.disk_usage('.').free
print(f'Available disk: {free / 1024**3:.1f} GiB')
if free < 64 * 1024**3:
    raise SystemExit('OS qualification requires 64 GiB free. Select a larger GitHub-hosted runner with ANIMA_OS_RUNNER.')
PY
