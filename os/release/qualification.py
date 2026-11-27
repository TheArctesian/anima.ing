"""Check the VM evidence for the ISO and signed update being promoted."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

from receipt import fingerprint, read_candidate, signed_candidate

FRESH_REQUIRED = {
    'installed-candidate-generation',
    'installed-recorded-data-volume',
    'installed-storage-capacity',
    'installed-podman-to-docker-cutover-and-reboot',
    'installed-private-ssh-rdp-vnc',
    'installed-hosting-and-application-terminal',
    'installed-hosting-pressure-outage-and-recovery',
    'unselected-disk',
}
RAILPACK_REQUIRED = {
    'offline-install',
    'installed-candidate-generation',
    'railpack-node-build-export-run',
    'railpack-python-build-export-run',
    'railpack-rust-build-export-run',
    'railpack-unsupported-source-refusal',
    'clean-shutdown',
    'unselected-disk',
}
UPGRADE_REQUIRED = {
    'installed-baseline-ready-for-upgrade',
    'signed-https-download-staging-without-live-activation',
    'updated-system-health-and-persistence',
    'private-ssh-rdp-vnc-desktop-reboot-owner-choice',
    'owner-rollback-preserves-data',
    'unselected-disk',
}


def checked_candidate(folder, *, unsigned=False, test_tools=False):
    receipt = (read_candidate(folder, unsigned=True, test_tools=test_tools)
               if unsigned else signed_candidate(folder, test_tools=test_tools)[0])
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
    if receipt['sourceRevision'] != revision or receipt['sourceFingerprint'] != fingerprint():
        raise ValueError('Candidate source differs from the qualification checkout')
    return receipt


def preflight(folder, *, unsigned=False, test_tools=False):
    return checked_candidate(folder, unsigned=unsigned, test_tools=test_tools)['system']


def require_report(folder, lane, expected, required):
    verify_report(folder / f'{lane}.json', lane, expected, required)


def verify_report(path, lane, expected, required):
    report = json.loads(path.read_text())
    if not isinstance(report, dict) or any(report.get(key) != value for key, value in expected.items()):
        raise ValueError(f'{lane.capitalize()} report does not match the candidate or baseline')
    results = report.get('results')
    if not isinstance(results, list) or any(
            not isinstance(entry, dict) or entry.get('result') != 'Passed'
            or not isinstance(entry.get('case'), str) for entry in results):
        raise ValueError(f'{lane.capitalize()} report contains an unsuccessful or invalid case')
    cases = {entry['case'] for entry in results}
    if len(cases) != len(results) or not required.issubset(cases):
        raise ValueError(f'{lane.capitalize()} report requires every release check exactly once')


def frozen_harness():
    edits = subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=all',
                                     '--', 'os/release', 'os/tests/vm', 'os/updates/feed.py'])
    if edits:
        raise ValueError('Qualification harness differs from the candidate checkout')
    harness = Path(__file__).resolve().parents[1] / 'tests' / 'vm'
    root = Path(subprocess.check_output(['git', 'rev-parse', '--show-toplevel'], text=True).strip()).resolve()
    if harness != root / 'os/tests/vm':
        raise ValueError('Run installed qualification with the candidate checkout’s scripts')
    return harness


def run_unsigned(folder, lane, output):
    harness = frozen_harness()
    fresh = lane == 'fresh-install'
    receipt = checked_candidate(folder, unsigned=True, test_tools=fresh)
    script = 'gui_test.py' if fresh else 'railpack_test.py'
    command = [sys.executable, harness / script, folder / 'anima-os-x86_64.iso',
               output, '--expected-system', receipt['system']]
    if fresh:
        command.extend(['--private-access-tools', folder / 'package'])
    subprocess.run(command, check=True)
    verify_report(output / 'results.json', lane, {
        'iso_sha256': receipt['isoSha256'], 'installed_system': receipt['system'],
    }, FRESH_REQUIRED if fresh else RAILPACK_REQUIRED)


def qualified_baseline(folder, receipt, release):
    proof = folder / 'qualification'
    candidate = {
        'iso_sha256': receipt['isoSha256'],
        'installed_system': release['system'],
    }
    require_report(proof, 'fresh-install', candidate, FRESH_REQUIRED)
    require_report(proof, 'railpack', candidate, RAILPACK_REQUIRED)
    manifest = (folder / 'baseline.json').read_bytes()
    baseline = json.loads(manifest)
    require_report(proof, 'upgrade', {
        'iso_sha256': baseline['isoSha256'],
        'baseline_system': baseline['system'],
        'baseline_manifest_sha256': hashlib.sha256(manifest).hexdigest(),
        'update_system': release['system'],
        'update_sha256': release['sha256'],
    }, UPGRADE_REQUIRED)
    return baseline


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('folder', type=Path)
    parser.add_argument('--unsigned', action='store_true')
    parser.add_argument('--test-tools', action='store_true')
    parser.add_argument('--lane', choices=['fresh-install', 'railpack'])
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if (args.lane or args.output) and not (args.lane and args.output and args.unsigned):
        parser.error('--lane and --output must be used together with --unsigned')
    if args.lane and args.test_tools:
        parser.error('--test-tools is for preflight without a lane')
    try:
        if args.lane:
            run_unsigned(args.folder, args.lane, args.output)
        else:
            print(preflight(args.folder, unsigned=args.unsigned, test_tools=args.test_tools))
    except (OSError, KeyError, ValueError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
