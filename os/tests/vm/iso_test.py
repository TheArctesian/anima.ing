"""Boot an actual ISO, cancel safely, install offline, then boot its installed disk."""
import argparse
import json
import os
from pathlib import Path
import secrets
from installer_checks import install_request, install_selected, prepare_serial_disk
from vm import Machine, digest


def run(iso, output):
    disk = prepare_serial_disk(iso, output)
    sentinel = output / 'sentinel.raw'
    before = digest(sentinel)
    results = []
    password = secrets.token_hex(12)  # Disposable VM-only account, never a production credential.
    with open(os.open(output / 'fixture-password', os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), 'w') as fixture:
        fixture.write(password)
    machine = Machine(output, disk, boot_iso=iso)
    try:
        machine.expect(r'root@anima-live:[^\r\n]*#', timeout=240)
        machine.send('stty -echo')
        machine.command('busctl call org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus StartServiceByName su fi.w1.wpa_supplicant1 0; busctl get-property fi.w1.wpa_supplicant1 /fi/w1/wpa_supplicant1 fi.w1.wpa_supplicant1 Interfaces')
        results.append({'case': 'wifi-supplicant-dbus-activation', 'result': 'Passed'})
        machine.command("timeout 90 sh -c 'until DISPLAY=:0 XAUTHORITY=/home/anima/.Xauthority anima-display --list >/dev/null 2>&1; do sleep 1; done'; output=$(DISPLAY=:0 XAUTHORITY=/home/anima/.Xauthority anima-display --list | cut -f1 | head -1); DISPLAY=:0 XAUTHORITY=/home/anima/.Xauthority anima-display --save \"$output\"; test -s /run/anima-display.json", timeout=100)
        machine.command('systemctl stop display-manager; pkill -TERM -x calamares || true; sleep 1')
        machine.command('test -d /sys/firmware/efi; test ! -e /var/lib/anima-daemon/token')
        machine.command('parted -s /dev/vda mklabel gpt mkpart primary ext4 1MiB 65MiB; udevadm settle; mkfs.ext4 -F /dev/vda1; mkdir /mnt/blocked-target; mount -t ext4 /dev/vda1 /mnt/blocked-target', timeout=60)
        machine.command('anima-os-install inventory | jq -e \'map(select(.path == "/dev/vda" or .path == "/dev/vdb")) | length == 2 and any(.[]; .path == "/dev/vda" and (.eligible | not)) and any(.[]; .path == "/dev/vdb" and .eligible)\'')
        machine.command('umount /mnt/blocked-target; wipefs --all /dev/vda; udevadm settle')
        results.append({'case': 'mounted-child-excluded', 'result': 'Passed'})
        machine.command('head -c 1048576 /dev/vda | sha256sum > /run/before-disk')
        results.append({'case': 'iso-uefi-live', 'result': 'Passed'})
        install_request(machine, password)
        machine.command('if anima-install < /run/install-unconfirmed.json; then exit 1; fi; head -c 1048576 /dev/vda | sha256sum | diff /run/before-disk -')
        results.append({'case': 'cancel-before-erase', 'result': 'Passed'})
        install_selected(machine)
        results.append({'case': 'offline-install', 'result': 'Passed'})
        machine.send('systemctl poweroff')
        machine.wait_exit(timeout=90)
        machine.close()
        machine = Machine(output, disk)  # ISO removed: this must boot the installed disk.
        machine.expect('anima(?:-node-[a-z0-9]{4})? login:', timeout=240)
        machine.send('admin')
        machine.expect('Password:'); machine.send(password)
        machine.shell('admin')
        machine.send('stty -echo')
        machine.command("findmnt -n -o OPTIONS / | tr ',' '\\n' | grep -qx ro; test -w /home/admin; ! touch /host-write-test")
        machine.command('echo retained > ~/iso-sentinel; cp /etc/machine-id ~/machine-id-before; command -v anima-daemon; command -v podman; ! systemctl is-active anima-daemon')
        machine.command('if anima-daemon >/tmp/usage 2>&1; then exit 1; else test "$?" = 2; fi; grep -q usage /tmp/usage')
        results.append({'case': 'installed-login-readonly-runtime', 'result': 'Passed'})
        from console_checks import installed_wifi, installed_orientation, installed_terminal_prompts
        installed_wifi(machine, password)
        installed_orientation(machine)
        installed_terminal_prompts(machine)
        results.append({'case': 'installed-terminal-prompt-profiles', 'result': 'Passed'})
        results.append({'case': 'confirmed-orientation-transferred-and-applied', 'result': 'Passed'})
        results.append({'case': 'installed-wifi-backend-after-root-lock', 'result': 'Passed'})
        from container_checks import enable_installed_hosting, installed_docker
        machine.send('sudo -k -i'); machine.expect(r'password for admin:'); machine.send(password)
        machine.shell('root')
        installed_docker(machine)
        enable_installed_hosting(machine)
        results.append({'case': 'first-run-hosting-settings-and-service', 'result': 'Passed'})
        from build_checks import installed_build
        installed_build(machine)
        results.append({'case': 'rootless-offline-build-verify-release', 'result': 'Passed', 'limitation': 'Synthetic Dockerfile only; no Railpack detection, secrets, network or deployment'})
        from build_lease_checks import installed_build_leases
        installed_build_leases(machine)
        results.append({'case': 'build-lease-renewal-expiry-and-watchdog-stop', 'result': 'Passed', 'limitation': 'Local owner API only; no daemon claim/lease integration'})
        machine.send('exit'); machine.shell('admin')
        results.append({'case': 'docker-hosting-slice-live-restore-and-isolation', 'result': 'Passed', 'limitation': 'Offline local Docker only; no hosted application orchestration or continuous availability measurement'})
        machine.command("podman info --format '{{.Host.Security.Rootless}}' | grep -qx true", timeout=60)
        machine.command('podman load -i /etc/anima-smoke-image.tar.gz', timeout=120)
        limits = '--pull=never --network=none --read-only --memory=64m --cpus=1 --cap-drop=ALL --security-opt=no-new-privileges'
        machine.command(f'podman run -d --name anima-smoke {limits} anima-preview-smoke:local /bin/httpd -f -p 8080 -h /www', timeout=60)
        machine.command('podman exec anima-smoke /bin/wget -qO- http://127.0.0.1:8080 | grep -qx anima-app-ok; podman rm -f anima-smoke', timeout=60)
        machine.command(f'podman run --rm {limits} anima-preview-smoke:local /bin/sh -c "! touch /write-test && echo anima-job-ok" | grep -qx anima-job-ok', timeout=60)
        results.append({'case': 'rootless-isolated-app-and-job', 'result': 'Passed', 'limitation': 'Local Podman only, not Anima orchestration or an agent run'})
        from console_checks import installed_console
        installed_console(machine, output, password)
        results.append({'case': 'installed-gui-auth-lock-network-display-fixed-footer', 'result': 'Passed'})
        machine.send('sudo systemctl reboot'); machine.expect(r'password for admin:'); machine.send(password)
        machine.expect('anima(?:-node-[a-z0-9]{4})? login:', timeout=240); machine.send('admin')
        machine.expect('Password:'); machine.send(password)
        machine.shell('admin'); machine.send('stty -echo')
        machine.command('grep -qx retained ~/iso-sentinel; cmp /etc/machine-id ~/machine-id-before')
        installed_orientation(machine)
        results.append({'case': 'installed-persistence', 'result': 'Passed'})
    except Exception as error:
        results.append({'case': 'iso-run', 'result': 'Failed', 'reason': str(error)})
    finally:
        machine.close()
        after = digest(sentinel)
        results.append({'case': 'unselected-disk', 'result': 'Passed' if before == after else 'Failed'})
        (output / 'results.json').write_text(json.dumps({'iso_sha256': digest(iso), 'results': results}, indent=2)+'\n')
    if any(result['result'] != 'Passed' for result in results):
        raise SystemExit(1)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('iso', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    run(args.iso.resolve(), args.output.resolve())
