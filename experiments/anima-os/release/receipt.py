"""Record the source and matching artifacts from one completed build."""
import json
from pathlib import Path
import subprocess
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'updates'))
from feed import digest, verify


def fingerprint():
    import hashlib
    tree = subprocess.check_output(['git', 'ls-tree', '-r', 'HEAD', '--',
                                    'experiments/anima-os', 'crates', 'link', 'Cargo.toml',
                                    'Cargo.lock', 'rust-toolchain.toml', '.cargo', '.github/workflows/os-release.yml'])
    return hashlib.sha256(tree).hexdigest()


def write(folder):
    release = verify((folder / 'package/latest.json').read_bytes())
    if release['system'] != str((folder / 'system').resolve()):
        raise ValueError('ISO system and update system differ')
    receipt = dict(version=release['version'], sourceRevision=subprocess.check_output(
        ['git', 'rev-parse', 'HEAD'], text=True).strip(), sourceFingerprint=fingerprint(),
        system=release['system'], isoSha256=digest(folder / 'anima-os-x86_64.iso'))
    (folder / 'release.json').write_text(json.dumps(receipt, indent=2) + '\n')


if __name__ == '__main__':
    write(Path(sys.argv[1]))
