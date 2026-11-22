"""Real EFI reboot acceptance for the Rust updater on disposable fixture disks.

The headless fixture supplies health dependencies; graphical appliance acceptance
remains separate. All keys, identity and workload records are synthetic.
"""
import argparse
import json
from pathlib import Path
import subprocess
import time

import update_fixture
from vm import Machine, digest, prepare


def ready(machine, revision):
    machine.expect('ANIMA_UPDATE_READY_' + revision, timeout=240)
    machine.expect(r'root@anima:[^\r\n]*#')
    machine.send('export PS1="ANIMA_PROMPT> "; stty -echo')
    machine.expect('ANIMA_PROMPT> ')


def run(fixture, output):
    disk, sentinel = prepare((fixture / 'anima.qcow2').resolve(), output)
    package = output / 'package'
    candidate = (fixture / 'system-b').read_text().strip()
    previous = (fixture / 'system-a').read_text().strip()
    subprocess.run(['python3', str(Path(__file__).parent / 'updates/package.py'),
                    candidate, str(package), '--key', str(fixture / 'keys/private.pem')], check=True)
    origin = update_fixture.prepare(package, output)
    machine = Machine(output, disk)
    results = []
    started = time.monotonic()
    try:
        ready(machine, 'A')
        machine.command("sha256sum /persist/identity/sentinel /persist/workload/sentinel > /persist/retained.sha256")
        update_fixture.attach(machine, output, origin)
        prefix = 'SSL_CERT_FILE=/run/update-origin/certificate.pem anima-os-update '
        machine.command(prefix + "check | jq -e '.ok and .value.available'")
        machine.command(prefix + "download | jq -e '.ok and .value.phase == \"staged\"'", timeout=600)
        machine.command('test "$(readlink -f /run/current-system)" = ' + previous)
        machine.command('test -f /boot/loader/entries/anima-candidate+2.conf')
        results.append({'case': 'signed-download-stages-without-switching', 'result': 'Passed'})
        machine.send('anima-os-update activate')
        ready(machine, 'B')
        machine.command('test "$(readlink -f /run/current-system)" = ' + candidate)
        machine.command("anima-os-update status | jq -e '.ok and .value.phase == \"current\"'")
        machine.command('test -f /boot/loader/entries/anima-candidate.conf; sha256sum -c /persist/retained.sha256')
        machine.command("findmnt -n -o OPTIONS / | tr ',' '\\n' | grep -qx ro")
        results.append({'case': 'activate-reboots-candidate-and-blesses-health', 'result': 'Passed'})
        machine.send('anima-os-update rollback')
        ready(machine, 'A')
        machine.command('test "$(readlink -f /run/current-system)" = ' + previous)
        machine.command("anima-os-update status | jq -e '.ok and .value.phase == \"rolled-back\"'")
        machine.command('sha256sum -c /persist/retained.sha256')
        results.append({'case': 'explicit-rollback-reboots-previous-preserves-data', 'result': 'Passed'})
        update_fixture.serve(machine)
        machine.command(prefix + "download | jq -e '.ok and .value.phase == \"staged\"'", timeout=600)
        machine.command("sed -i '/^options /s/$/ systemd.mask=display-manager.service/' /boot/loader/entries/anima-candidate+2.conf; sync")
        machine.send('anima-os-update activate')
        ready(machine, 'A')
        machine.command('test "$(readlink -f /run/current-system)" = ' + previous)
        machine.command("anima-os-update status | jq -e '.ok and .value.phase == \"rolled-back\"'")
        machine.command('test -f /boot/loader/entries/anima-candidate+0-2.conf; sha256sum -c /persist/retained.sha256')
        results.append({'case': 'failed-health-exhausts-two-boots-and-recovers-previous', 'result': 'Passed'})
        machine.send('systemctl poweroff')
        machine.wait_exit()
    except Exception as error:
        results.append({'case': 'boot-acceptance', 'result': 'Failed', 'reason': str(error)})
        raise
    finally:
        machine.close()
        after = digest(output / 'sentinel.raw')
        results.append({'case': 'unselected-disk-unchanged', 'result': 'Passed' if after == sentinel else 'Failed'})
        (output / 'update-boot-results.json').write_text(json.dumps({
            'seconds': time.monotonic() - started, 'candidate': candidate,
            'previous': previous, 'results': results,
        }, indent=2) + '\n')
    if any(result['result'] != 'Passed' for result in results):
        raise RuntimeError('Boot acceptance failed; see update-boot-results.json')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('fixture', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    run(args.fixture.resolve(), args.output.absolute())
