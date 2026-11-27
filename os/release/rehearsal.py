"""Qualify a frozen pre-move upgrade using the disposable build's verification key."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

from qualification import UPGRADE_REQUIRED, checked_candidate, frozen_harness, verify_report
from receipt import digest, read_candidate, signed_update


def verify_rehearsal(folder):
    candidate_folder, baseline_folder = folder / 'candidate', folder / 'baseline'
    candidate = checked_candidate(candidate_folder, unsigned=True, test_tools=True)
    baseline = read_candidate(baseline_folder, unsigned=True)
    old, new = baseline['sourceRevision'], candidate['sourceRevision']
    if old == new or not re.fullmatch(r'[a-f0-9]{40}', old):
        raise ValueError('The upgrade rehearsal requires a different pre-move revision')
    subprocess.run(['git', 'merge-base', '--is-ancestor', old, new], check=True)
    subprocess.run(['git', 'cat-file', '-e', f'{old}:experiments/anima-os/flake.nix'], check=True)
    key = folder / 'fixture-public.pem'
    key_hash = digest(key)
    for image in (baseline_folder, candidate_folder):
        inputs = json.loads((image / 'build-inputs.json').read_bytes())
        if inputs.get('fixturePublicKeySha256') != key_hash:
            raise ValueError('The images do not share the recorded rehearsal key')
    release = signed_update(candidate_folder, key=key)
    if release['system'] != candidate['system'] or release['system'] == baseline['system']:
        raise ValueError('The rehearsal update and ISO must contain the same new generation')
    return baseline, release, key_hash


def run_upgrade(folder, output):
    harness = frozen_harness()
    baseline, release, key_hash = verify_rehearsal(folder)
    manifest = folder / 'baseline' / 'candidate.json'
    subprocess.run([
        sys.executable, harness / 'update_test.py', folder / 'baseline' / 'anima-os-x86_64.iso',
        folder / 'candidate' / 'package', output, '--remote',
        '--baseline-manifest', manifest, '--fixture-public-key', folder / 'fixture-public.pem',
    ], check=True)
    verify_report(output / 'results.json', 'upgrade', {
        'iso_sha256': baseline['isoSha256'],
        'baseline_system': baseline['system'],
        'baseline_manifest_sha256': digest(manifest),
        'update_system': release['system'],
        'update_sha256': release['sha256'],
        'fixture_public_key_sha256': key_hash,
    }, UPGRADE_REQUIRED)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('folder', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    try:
        if args.output is None:
            frozen_harness()
            _, release, _ = verify_rehearsal(args.folder)
            print(release['system'])
        else:
            run_upgrade(args.folder, args.output)
    except (OSError, KeyError, ValueError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
