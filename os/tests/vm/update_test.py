"""Install the actual ISO, update from signed HTTPS bytes, reboot and roll back without data loss."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import shlex
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from updates.feed import verify
from gui_test import prepare, graphical_install, screenshot, eject_install_media
from gui_actions import tap
from storage_checks import check_data_volume
from vm import Machine, digest
import update_fixture
import vm_host


def root(machine,password):
    machine.send('sudo -k -i');machine.expect('password for admin:');machine.send(password)
    machine.shell('root');machine.send('stty -echo')


def login(machine,password):
    boot_timeout = 420 if vm_host.accelerator() == 'tcg' else 240
    machine.expect('anima(?:-node-[a-z0-9]{4})? login:',timeout=boot_timeout);machine.send('admin')
    machine.expect('Password:');machine.send(password)
    machine.shell('admin');machine.send('stty -echo')
    root(machine,password)


def check_fixture_key(machine, expected):
    if expected is not None:
        checksum = shlex.quote(expected + '  /run/current-system/anima-rehearsal-public.pem')
        machine.command(f"printf '%s\\n' {checksum} | sha256sum -c -")


def run(args):
    iso, package, output = (path.resolve() for path in (args.iso, args.package, args.output))
    envelope = (package/'latest.json').read_bytes()
    candidate = verify(envelope) if args.fixture_public_key is None else verify(envelope, key=args.fixture_public_key)
    fixture_key_hash = digest(args.fixture_public_key) if args.fixture_public_key is not None else None
    expected = shlex.quote(candidate['system'])
    baseline_manifest = args.baseline_manifest.read_bytes() if args.baseline_manifest else None
    baseline = json.loads(baseline_manifest) if baseline_manifest is not None else None
    iso_sha256 = digest(iso)
    if baseline and 'isoSha256' in baseline and baseline['isoSha256'] != iso_sha256:
        raise ValueError('The ISO differs from the verified upgrade baseline')
    if baseline and baseline['system'] == candidate['system']:
        raise ValueError('An upgrade must exercise two different generations')
    if args.resume:
        disk=output/'installed.qcow2';fixture=output/'update-origin.raw'
        password=(output/'fixture-password').read_text()
    else:
        disk=prepare(output);fixture=update_fixture.prepare(package,output)
        password=secrets.token_hex(12)
        with open(os.open(output/'fixture-password',os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600),'w') as secret:
            secret.write(password)
    with_data_volume = (output/'data.raw').is_file()
    before=digest(output/'sentinel.raw')
    results=[];machine=Machine(output,disk,boot_iso=None if args.resume else iso)
    try:
        if not args.resume:
            graphical_install(machine,output,password)
            tap(machine,output,'Restart')
            machine.expect('reboot: Restarting system', timeout=90)
            eject_install_media(output)
        login(machine,password)
        check_fixture_key(machine, fixture_key_hash)
        if baseline is not None:
            machine.command('test "$(readlink -f /run/current-system)" = '+shlex.quote(baseline['system']))
        if with_data_volume:
            check_data_volume(machine)
        results.append({'case':'installed-baseline-ready-for-upgrade','result':'Passed'})
        machine.command('readlink -f /run/current-system > /persist/update-before; cp /etc/machine-id /persist/update-identity; cp /etc/hostname /persist/update-hostname; echo keep-work > /persist/update-work')
        update_fixture.attach(machine,output,fixture)
        machine.command('SSL_CERT_FILE=/run/update-origin/certificate.pem anima-os-update check > /tmp/check.json || { cat /tmp/check.json; journalctl -u anima-update-fixture --no-pager -n 12; exit 1; }; jq -e ".ok and .value.available" /tmp/check.json',timeout=45)
        machine.command('SSL_CERT_FILE=/run/update-origin/certificate.pem anima-os-update download > /tmp/stage.json; '
                        f'jq -e --arg system {expected} '
                        "'.ok and .value.phase == \"staged\" and .value.system == $system' /tmp/stage.json",
                        timeout=1800)
        machine.command('cmp /persist/update-before <(readlink -f /run/current-system); grep -qx keep-work /persist/update-work')
        results.append({'case':'signed-https-download-staging-without-live-activation','result':'Passed'})
        machine.send('anima-os-update activate')
        login(machine,password)
        check_fixture_key(machine, fixture_key_hash)
        machine.command(f'test "$(readlink -f /run/current-system)" = {expected}')
        current_filter = shlex.quote('.ok and .value.phase == "current"')
        wait_for_health = f'until anima-os-update status | jq -e {current_filter}; do sleep 2; done'
        machine.command('timeout 90 sh -c '+shlex.quote(wait_for_health),timeout=95)
        machine.command('cmp /persist/update-identity /etc/machine-id; cmp /persist/update-hostname /proc/sys/kernel/hostname; grep -qx keep-work /persist/update-work; findmnt -n -o OPTIONS / | tr , "\\n" | grep -qx ro')
        if with_data_volume:
            check_data_volume(machine)
        results.append({'case':'updated-system-health-and-persistence','result':'Passed'})
        from console_checks import wait_label
        wait_label(output, 'Local session locked')
        screenshot(output, 'updated-console-locked')
        results.append({'case':'updated-graphical-console','result':'Passed'})
        if args.remote:
            from remote_checks import check
            update_fixture.mount(machine)
            check(machine,password,package)
            results.append({'case':'private-ssh-rdp-vnc-desktop-reboot-owner-choice','result':'Passed'})
        machine.send('anima-os-update rollback')
        login(machine,password)
        check_fixture_key(machine, fixture_key_hash)
        machine.command('cmp /persist/update-before <(readlink -f /run/current-system); cmp /persist/update-identity /etc/machine-id; cmp /persist/update-hostname /proc/sys/kernel/hostname; grep -qx keep-work /persist/update-work')
        if with_data_volume:
            check_data_volume(machine)
        results.append({'case':'owner-rollback-preserves-data','result':'Passed'})
    except Exception as error:
        results.append({'case':'update-run','result':'Failed','reason':str(error)})
        try:
            machine.command('cat /tmp/check.json /tmp/stage.json 2>/dev/null || true; journalctl -b -u anima-update-health -u anima-update-fixture --no-pager -n 20')
        except Exception:
            pass
        if machine.process.poll() is None:
            try:
                screenshot(output,'failure')
            except (OSError, RuntimeError):
                pass
    finally:
        machine.close()
        results.append({'case':'unselected-disk','result':'Passed' if digest(output/'sentinel.raw')==before else 'Failed'})
        (output/'results.json').write_text(json.dumps({
            'iso_sha256':iso_sha256, 'baseline_system': baseline['system'] if baseline else None,
            'baseline_manifest_sha256': hashlib.sha256(baseline_manifest).hexdigest() if baseline_manifest is not None else None,
            'update_system':candidate['system'], 'update_sha256':candidate['sha256'], 'results':results,
            'fixture_public_key_sha256': fixture_key_hash,
        },indent=2)+'\n')
    if any(entry['result']!='Passed' for entry in results):
        raise SystemExit(1)


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('iso',type=Path);parser.add_argument('package',type=Path);parser.add_argument('output',type=Path)
    parser.add_argument('--resume',action='store_true')
    parser.add_argument('--remote',action='store_true')
    parser.add_argument('--baseline-manifest',type=Path)
    parser.add_argument('--fixture-public-key',type=Path)
    run(parser.parse_args())
