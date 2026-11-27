"""Check the staged local crate closure using the workspace lockfile."""
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[2]
CONTROLLERS = (
    'anima-applications', 'anima-osd', 'anima-os-install', 'anima-os-session',
    'anima-os-update', 'anima-os-maintenance', 'anima-os-execution',
)


def local_dependencies(lock, controllers):
    packages = {}
    for entry in lock['package']:
        packages.setdefault(entry['name'], []).append(entry)
    pending = list(controllers)
    seen = set()
    while pending:
        reference = pending.pop()
        if reference in seen:
            continue
        seen.add(reference)
        name, *version = reference.split()
        package, = [entry for entry in packages[name]
                    if not version or entry['version'] == version[0]]
        pending.extend(package.get('dependencies', ()))
        if 'source' not in package:
            yield name


class Projection(unittest.TestCase):
    def test_projected_controllers_include_all_locked_local_dependencies(self):
        lock = tomllib.loads((ROOT / 'Cargo.lock').read_text())
        expected = set(local_dependencies(lock, CONTROLLERS))
        with tempfile.TemporaryDirectory() as folder:
            payload = Path(folder) / 'source'
            subprocess.run(['bash', ROOT / 'os/release/project-sources.sh', payload],
                           cwd=folder, check=True)
            projected = {manifest.parent.name for manifest in payload.glob('crates/*/Cargo.toml')}
            self.assertEqual(projected, expected)
            for name in expected:
                self.assertTrue((payload / 'crates' / name / 'src').is_dir(), name)


if __name__ == '__main__':
    unittest.main()
