"""Qualify public dependency builds from an ISO or a disposable installed clone."""
import argparse
import json
from pathlib import Path
import secrets
from gui_test import require_system
from installer_checks import install_request, install_selected, prepare_serial_disk
from remote_test import clone
from update_test import login
from vm import Machine, digest
import vm_host
from railpack_checks import BUILD_MEMORY_MIB, PROJECTS, installed_railpack, installed_unsupported_railpack


GUEST_MEMORY_MIB = 10_240


def prepare_clone(installed, output):
    previous = json.loads((installed / 'results.json').read_text())
    if not any(row['case'] == 'offline-install' and row['result'] == 'Passed'
               for row in previous['results']):
        raise ValueError('Use a completed disposable installation fixture.')
    output.mkdir(parents=True, exist_ok=False)
    vm_host.require_storage(output, 64)
    vm_host.copy_disk(installed / 'installed.qcow2', output / 'installed.qcow2')
    for name in ['vars.fd', 'sentinel.raw']:
        clone(installed / name, output / name)
    return previous['iso_sha256'], (installed / 'fixture-password').read_text()


def install_offline(iso, output, password):
    machine = Machine(output, output / 'installed.qcow2', boot_iso=iso)
    try:
        machine.expect(r'root@anima-live:[^\r\n]*#', timeout=240)
        machine.send('stty -echo')
        machine.command('systemctl stop display-manager; pkill -TERM -x calamares || true; sleep 1')
        install_request(machine, password)
        install_selected(machine)
        machine.send('systemctl poweroff')
        machine.wait_exit(timeout=90)
    finally:
        machine.close()


def failure(results, case, error, password):
    reason = str(error).replace(password, '[fixture password]') if password else str(error)
    results.append({'case': case, 'result': 'Failed', 'reason': reason})


def build_online(output, password, expected_system, results):
    machine = Machine(output, output / 'installed.qcow2', network=True, memory_mib=GUEST_MEMORY_MIB)
    language = 'readiness'
    authenticated = False
    installed_system = None
    try:
        login(machine, password)
        authenticated = True
        if expected_system is not None:
            require_system(machine, expected_system)
            installed_system = expected_system
            results.append({'case': 'installed-candidate-generation', 'result': 'Passed'})
        machine.command('test ! -e /var/lib/anima-daemon/token; ! systemctl is-active --quiet anima-daemon')
        machine.command("timeout 180 sh -c 'until systemctl is-active --quiet anima-execution-recovery; do sleep 2; done'", timeout=190)
        machine.command("timeout 600 sh -c 'until systemctl is-active --quiet anima-workspace-image; do systemctl is-failed --quiet anima-workspace-image && exit 1; sleep 2; done'", timeout=610)
        machine.command('curl --connect-timeout 10 --max-time 30 --silent --show-error --head https://ghcr.io/v2/ >/dev/null', timeout=40)
        language = 'unsupported-source'
        installed_unsupported_railpack(machine)
        results.append({'case': 'railpack-unsupported-source-refusal', 'result': 'Passed'})
        for language in PROJECTS:
            installed_railpack(machine, language)
            results.append({'case': 'railpack-' + language + '-build-export-run', 'result': 'Passed'})
    except Exception as error:
        failure(results, 'railpack-' + language, error, password)
    finally:
        if authenticated and machine.process.poll() is None:
            try:
                machine.send('systemctl poweroff')
                machine.wait_exit(timeout=90)
                results.append({'case': 'clean-shutdown', 'result': 'Passed'})
            except Exception as error:
                failure(results, 'clean-shutdown', error, password)
        machine.close()
    return installed_system


def run(source, output, *, expected_system=None):
    from_iso = not source.is_dir()
    if from_iso:
        prepare_serial_disk(source, output)
        iso_sha256, password = digest(source), secrets.token_hex(12)
    else:
        iso_sha256, password = prepare_clone(source, output)
    before = digest(output / 'sentinel.raw')
    results = []
    installed_system = None
    try:
        if from_iso:
            install_offline(source, output, password)
            results.append({'case': 'offline-install', 'result': 'Passed'})
        installed_system = build_online(output, password, expected_system, results)
    except Exception as error:
        failure(results, 'railpack-run', error, password)
    results.append({'case': 'unselected-disk',
                    'result': 'Passed' if digest(output / 'sentinel.raw') == before else 'Failed'})
    (output / 'results.json').write_text(json.dumps({
        'iso_sha256': iso_sha256, 'installed_system': installed_system,
        'memory_mib': GUEST_MEMORY_MIB, 'build_memory_mib': BUILD_MEMORY_MIB,
        'scope': 'Synthetic public dependency builds and test-only Docker import and run in a disposable guest.',
        'results': results,
    }, indent=2) + '\n')
    if any(row['result'] != 'Passed' for row in results):
        raise SystemExit(1)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path, help='Candidate ISO or completed serial installation directory')
    parser.add_argument('output', type=Path)
    parser.add_argument('--expected-system')
    args = parser.parse_args()
    run(args.source.resolve(), args.output.resolve(), expected_system=args.expected_system)
