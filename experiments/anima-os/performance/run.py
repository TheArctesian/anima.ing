"""Compare actual live ISOs and their installed shells on fresh offline VM disks."""
import argparse
import base64
import json
import os
import platform
import re
from pathlib import Path
import secrets
import shlex
import shutil
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from vm import Machine, digest
from gui_test import screenshot
from gui_actions import locate
from installer_checks import install_request


def prepare(folder):
    folder.mkdir(parents=True, exist_ok=False)
    if subprocess.check_output(['stat', '-f', '-c', '%T', str(folder)], text=True).strip() in ('tmpfs', 'ramfs'):
        raise ValueError('Use disk-backed storage')
    if shutil.disk_usage(folder).free < 64 * 1024**3:
        raise ValueError('64 GiB free required')
    disk = folder / 'installed.qcow2'
    subprocess.run(['qemu-img', 'create', '-f', 'qcow2', str(disk), '64G'], check=True)
    shutil.copy('/usr/share/edk2/x64/OVMF_VARS.4m.fd', folder / 'vars.fd')
    with (folder / 'sentinel.raw').open('wb') as stream:
        stream.write(b'ANIMA_PERFORMANCE_UNSELECTED')
        stream.truncate(1024**3)
    return disk


def samples(machine, mode):
    code = base64.b64encode(Path(__file__).with_name('guest.py').read_bytes()).decode()
    offset = len(machine.buffer)
    resolve = 'mapfile -t perf_python < <(for candidate in /nix/store/*-python3-*/bin/python3; do case "$candidate" in *-env/bin/python3) continue;; esac; if test -x "$candidate"; then readlink -f "$candidate"; fi; done | sort -u); test "${#perf_python[@]}" = 1'
    machine.command(f'{resolve}; echo {shlex.quote(code)} | base64 -d > /tmp/anima-perf-guest.py; timeout 120 "${{perf_python[0]}}" /tmp/anima-perf-guest.py {mode}', timeout=130)
    match = re.search(r'ANIMA_PERFORMANCE=(\{[^\r\n]+\})', machine.buffer[offset:])
    if not match:
        raise ValueError('Guest did not produce measurement JSON')
    return json.loads(match.group(1))


def installed_login(machine, password):
    machine.expect('anima(?:-node-[a-z0-9]{4})? login:', timeout=240)
    machine.send('admin')
    machine.expect('Password:')
    machine.send(password)
    machine.expect(r'admin@anima(?:-node-[a-z0-9]{4})?:[^\r\n]*\$')
    machine.send('stty -echo')
    machine.send('sudo -i')
    machine.expect('password for admin:')
    machine.send(password)
    machine.expect(r'root@anima(?:-node-[a-z0-9]{4})?:[^\r\n]*#')
    machine.send('stty -echo')


def install(machine, password):
    offset = len(machine.buffer)
    machine.command('if command -v anima-os-install >/dev/null; then echo PERF_NATIVE; else echo PERF_LEGACY; fi')
    if re.search(r'PERF_NATIVE(?:\r*\n)', machine.buffer[offset:]):
        install_request(machine, password)
        machine.command('anima-install < /run/install.json > /run/install-progress.jsonl; grep -q ANIMA_INSTALL_COMPLETE /run/install-progress.jsonl', timeout=1200)
        return
    machine.send('anima-install')
    machine.expect('Disk number')
    machine.send('1')
    machine.expect('Password for the local admin account')
    machine.send(password)
    machine.expect('Repeat password:')
    machine.send(password)
    machine.expect('type exactly ERASE /dev/vda:')
    machine.send('ERASE /dev/vda')
    finish = machine.expect(r'ANIMA_INSTALL_COMPLETE|Installation stopped:|Traceback \(most recent call last\):', timeout=1200)
    if finish.group(0) != 'ANIMA_INSTALL_COMPLETE':
        raise RuntimeError('Baseline installer failed; see serial.log')


def one(iso, folder):
    disk = prepare(folder)
    password = secrets.token_hex(12)
    result = dict(iso=str(iso), iso_bytes=iso.stat().st_size, iso_sha256=digest(iso), host_load_start=list(os.getloadavg()), host_meminfo_start=Path('/proc/meminfo').read_text())
    before = digest(folder / 'sentinel.raw')
    started = time.monotonic()
    machine = Machine(folder, disk, boot_iso=iso)
    try:
        machine.expect(r'root@anima-live:[^\r\n]*#', timeout=240)
        result['live_serial_shell_seconds'] = time.monotonic() - started
        machine.send('stty -echo')
        machine.command("timeout 90 sh -c 'until DISPLAY=:0 XAUTHORITY=/home/anima/.Xauthority xdotool search --onlyvisible --class calamares; do sleep 0.2; done'", timeout=100)
        result['live_visible_installer_seconds'] = time.monotonic() - started
        screenshot(folder, 'live-ready')
        result['live'] = samples(machine, 'live')
        machine.command('systemctl stop display-manager; pkill -TERM -x calamares || true')
        install(machine, password)
        machine.send('systemctl poweroff')
        machine.wait_exit()
        machine.close()
        started = time.monotonic()
        machine = Machine(folder, disk)
        installed_login(machine, password)
        result['installed_serial_root_seconds'] = time.monotonic() - started
        deadline = time.monotonic() + 90
        while True:
            try:
                locate(folder, 'Local session locked')
                break
            except ValueError:
                if time.monotonic() >= deadline:
                    raise TimeoutError('Installed lock screen not observed')
                time.sleep(.5)
        result['installed_locked_screen_observed_seconds'] = time.monotonic() - started
        screenshot(folder, 'installed-ready')
        result['installed'] = samples(machine, 'installed')
    except Exception as error:
        result['error'] = str(error)
        raise
    finally:
        machine.close()
        result['host_load_end'] = list(os.getloadavg())
        result['unselected_disk_unchanged'] = digest(folder / 'sentinel.raw') == before
        (folder / 'measurements.json').write_text(json.dumps(result, indent=2) + '\n')
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('before', type=Path)
    parser.add_argument('after', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--rounds', type=int, default=3)
    parser.add_argument('--resume', action='store_true', help='Reuse completed measurement records, never VM disks')
    args = parser.parse_args()
    args.before = args.before.resolve(strict=True)
    args.after = args.after.resolve(strict=True)
    if args.rounds < 1:
        parser.error('rounds must be positive')
    args.output.mkdir(parents=True, exist_ok=args.resume)
    (args.output / 'manifest.json').write_text(json.dumps(dict(
        qemu=subprocess.check_output(['qemu-system-x86_64', '--version'], text=True).splitlines()[0],
        host_kernel=platform.release(), guest=dict(cpus=2, memory_mib=4096, network='none', firmware='OVMF UEFI', disk='fresh64GiB qcow2'),
        rounds=args.rounds, settling_seconds=20, memory_samples=5, status_samples=10,
        background_workload='Host desktop and unrelated long-running Windows process retained; no host cache clearing or workload isolation',
        caveats='ISO digest warms host file cache before each boot; graphical times are observed upper bounds, include serial login/polling/OCR; RSS double-counts shared pages'
    ), indent=2) + '\n')
    records = json.loads((args.output / 'results.json').read_text()) if args.resume else []
    for record in records:
        expected = args.before if record['label'] == 'before' else args.after
        if record['iso'] != str(expected):
            raise ValueError('Resume artifact does not match existing records')
    for index in range(args.rounds):
        order = [('before', args.before), ('after', args.after)]
        for label, iso in order if index % 2 == 0 else reversed(order):
            if any(record['round'] == index+1 and record['label'] == label for record in records):
                continue
            folder = args.output / f'{index+1}-{label}'
            if folder.exists():
                folder.rename(folder.with_name(folder.name + '-incomplete-' + str(time.monotonic_ns())))
            result = one(iso.resolve(strict=True), folder)
            records.append(dict(round=index+1, label=label, **result))
            (args.output / 'results.json').write_text(json.dumps(records, indent=2) + '\n')


if __name__ == '__main__':
    main()
