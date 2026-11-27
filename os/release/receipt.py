"""Record the source and matching artifacts from one completed build."""
import json
from pathlib import Path
import subprocess
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'updates'))
from feed import digest, verify

PRIVATE_ACCESS_FILES = ('private-access.test', 'test-tools.json', 'test-tools.nar')


def private_access_hashes(folder):
    return {name: digest(folder / 'package' / name) for name in PRIVATE_ACCESS_FILES}


def fingerprint():
    import hashlib
    tree = subprocess.check_output(['git', 'ls-tree', '-r', 'HEAD', '--',
                                    'os', 'crates', 'link', 'Cargo.toml',
                                    'Cargo.lock', 'rust-toolchain.toml', '.cargo', 'quality/bin/rustc-cache.sh',
                                    '.github/workflows/os-release.yml'])
    return hashlib.sha256(tree).hexdigest()


def built(folder):
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
    inputs = folder / 'build-inputs.json'
    if json.loads(inputs.read_text())['sourceRevision'] != revision:
        raise ValueError('Generated build inputs belong to a different candidate')
    return dict(sourceRevision=revision, sourceFingerprint=fingerprint(),
                system=str((folder / 'system').resolve()),
                isoSha256=digest(folder / 'anima-os-x86_64.iso'), buildInputsSha256=digest(inputs))


def write(folder, unsigned=False, test_tools=False):
    receipt = built(folder)
    if test_tools:
        receipt['testToolsSha256'] = private_access_hashes(folder)
    if unsigned:
        path = folder / 'candidate.json'
    else:
        release = verify((folder / 'package/latest.json').read_bytes())
        if release['system'] != receipt['system']:
            raise ValueError('ISO system and update system differ')
        receipt['version'] = release['version']
        path = folder / 'release.json'
    path.write_text(json.dumps(receipt, indent=2) + '\n')


def read_candidate(folder, *, unsigned=False, test_tools=False):
    name = 'candidate.json' if unsigned else 'release.json'
    receipt = json.loads((folder / name).read_text())
    inputs = folder / 'build-inputs.json'
    if (digest(inputs) != receipt['buildInputsSha256'] or
            json.loads(inputs.read_text())['sourceRevision'] != receipt['sourceRevision']):
        raise ValueError('Generated build inputs differ from the candidate receipt')
    if digest(folder / 'anima-os-x86_64.iso') != receipt['isoSha256']:
        raise ValueError('Candidate ISO bytes differ from the build receipt')
    tools = receipt.get('testToolsSha256')
    if test_tools and tools is None:
        raise ValueError('Candidate has no recorded private-access test tools')
    if tools is not None and tools != private_access_hashes(folder):
        raise ValueError('Private-access test tools differ from the candidate receipt')
    return receipt


def signed_update(folder, *, key=None):
    envelope = (folder / 'package/latest.json').read_bytes()
    release = verify(envelope) if key is None else verify(envelope, key=key)
    archive = folder / 'package' / release['version'] / 'system.nar.xz'
    if archive.stat().st_size != release['bytes'] or digest(archive) != release['sha256']:
        raise ValueError('Candidate update bytes differ from the signed package')
    return release


def signed_candidate(folder, *, test_tools=False):
    receipt = read_candidate(folder, test_tools=test_tools)
    release = signed_update(folder)
    if receipt['version'] != release['version'] or receipt['system'] != release['system']:
        raise ValueError('Build provenance does not match signed package')
    return receipt, release


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('folder', type=Path)
    parser.add_argument('--unsigned', action='store_true')
    parser.add_argument('--test-tools', action='store_true')
    args = parser.parse_args()
    write(args.folder, args.unsigned, args.test_tools)
