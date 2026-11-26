"""Run existing remote acceptance against a clone, without signing or updating an OS."""
import argparse
import json
from pathlib import Path
import platform
import subprocess
from gui_test import screenshot
from remote_checks import TOOL_FILES, check
from update_test import login
from vm import Machine, digest
import vm_host
import update_fixture


def clone(source, target):
    arguments = ['cp', '-c'] if platform.system() == 'Darwin' else ['cp', '--reflink=auto', '--sparse=always']
    subprocess.run([*arguments, str(source), str(target)], check=True)
    target.chmod(0o600)


def run(installed, package, fixture, output):
    required = ['installed.qcow2', 'vars.fd', 'sentinel.raw', 'fixture-password', 'results.json']
    if any(not (installed / name).is_file() for name in required):
        raise ValueError('Use a completed disposable installation fixture, with its synthetic password.')
    previous = json.loads((installed / 'results.json').read_text())
    if not any(entry['case'] == 'offline-install' and entry['result'] == 'Passed'
               for entry in previous['results']):
        raise ValueError('The source fixture has no successful offline installation.')
    if not fixture.is_file() or any(not (package / name).is_file()
                                    for name in TOOL_FILES):
        raise ValueError('Prepare the offline remote tool package and fixture disk first.')
    output.mkdir(parents=True, exist_ok=False)
    vm_host.require_storage(output, 64)
    # Conversion holds QEMU's source-image lock; a running source VM is refused.
    subprocess.run(['qemu-img', 'convert', '-f', 'qcow2', '-O', 'qcow2',
                    str(installed / 'installed.qcow2'), str(output / 'installed.qcow2')], check=True)
    (output / 'installed.qcow2').chmod(0o600)
    for name in ['vars.fd', 'sentinel.raw']:
        clone(installed / name, output / name)
    clone(fixture, output / 'update-origin.raw')
    password = (installed / 'fixture-password').read_text()
    before = digest(output / 'sentinel.raw')
    results = []
    machine = Machine(output, output / 'installed.qcow2')
    try:
        login(machine, password)
        update_fixture.attach(machine, output, output / 'update-origin.raw', serve_https=False)
        check(machine, password, package)
        results.append({'case': 'private-ssh-rdp-vnc-shared-console-owner-choice', 'result': 'Passed',
                        'limitation': 'Synthetic private-link peers; Hub app sessions and active revocation are separate checks'})
    except Exception as error:
        results.append({'case': 'remote-run', 'result': 'Failed', 'reason': str(error)})
        if machine.process.poll() is None:
            screenshot(output, 'remote-failure')
    finally:
        machine.close()
        results.append({'case': 'unselected-disk',
                        'result': 'Passed' if digest(output / 'sentinel.raw') == before else 'Failed'})
        (output / 'results.json').write_text(json.dumps({
            'iso_sha256': previous['iso_sha256'],
            'fixture_sha256': digest(fixture),
            'scope': 'Installed remote access only; no update, rollback or publication',
            'results': results,
        }, indent=2) + '\n')
    if any(entry['result'] != 'Passed' for entry in results):
        raise SystemExit(1)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['installed', 'package', 'fixture', 'output']:
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    run(*(getattr(args, name).resolve() for name in ['installed', 'package', 'fixture', 'output']))
