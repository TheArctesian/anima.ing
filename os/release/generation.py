"""Bind an installed generation's cutover and console assets to the source commit."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

CONTAINERS = 'os/modules/containers.nix'
WORKLOADS = 'os/console/workloads.js'


def command(*arguments):
    return subprocess.check_output(arguments)


def checked_source(revision, path):
    try:
        committed = command('git', 'show', f'{revision}:{path}')
    except subprocess.CalledProcessError as error:
        raise ValueError(f'{path} is missing from candidate revision {revision}') from error
    if Path(path).read_bytes() != committed:
        raise ValueError(f'{path} differs from candidate revision {revision}')
    return committed


def member(closure, name):
    matches = [path for path in closure if path.name.partition('-')[2] == name]
    if len(matches) != 1:
        raise ValueError(f'expected one {name} output in the installed closure')
    return matches[0]


def verify(generation):
    revision = command('git', 'rev-parse', 'HEAD').decode().strip()
    checked_source(revision, CONTAINERS)
    workloads = checked_source(revision, WORKLOADS)
    marker = generation / 'anima-hosting-cutover-captured'
    if marker.read_bytes() != b'2\n':
        raise ValueError('installed generation does not declare cutover format 2')
    output = command('nix-store', '-qR', str(generation.resolve()))
    closure = [Path(path) for path in output.decode().splitlines()]
    provenance = member(closure, 'anima-os-source-provenance')
    if (provenance / 'share/anima-build/SOURCE-REVISION').read_text().strip() != revision:
        raise ValueError('installed generation source revision differs from the candidate')
    console = member(closure, 'anima-local-console-source')
    if (console / 'workloads.js').read_bytes() != workloads:
        raise ValueError('installed workloads.js differs from the candidate revision')
    print(f'Installed cutover and console verified for {revision}')


def record_inputs(destination, os_directory=Path('os'), fixture_key=None):
    payload = os_directory / 'payload'
    revision = command('git', 'rev-parse', 'HEAD').decode().strip()
    if (payload / 'SOURCE-REVISION').read_text().strip() != revision:
        raise ValueError('runtime payload source revision differs from the candidate')
    names = ('anima-daemon', 'anima-link', 'SOURCE-REVISION', 'SOURCE-SHA256',
             'CARGO-LOCK-SHA256', 'GO-SUM-SHA256', 'BINARY-SHA256', 'LINK-BINARY-SHA256')
    hashes = {}
    for name in names:
        with (payload / name).open('rb') as stream:
            hashes[name] = hashlib.file_digest(stream, 'sha256').hexdigest()
    manifest = dict(sourceRevision=revision, payloadSha256=hashes)
    if fixture_key is not None:
        with fixture_key.open('rb') as stream:
            manifest['fixturePublicKeySha256'] = hashlib.file_digest(stream, 'sha256').hexdigest()
    destination.write_text(json.dumps(manifest, indent=2) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('path', type=Path)
    parser.add_argument('--record-inputs', action='store_true')
    parser.add_argument('--os-directory', type=Path, default=Path('os'))
    parser.add_argument('--fixture-public-key', type=Path)
    args = parser.parse_args()
    try:
        if args.record_inputs:
            record_inputs(args.path, args.os_directory, args.fixture_public_key)
        else:
            verify(args.path)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
