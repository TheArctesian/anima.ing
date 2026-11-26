"""OS changes select a release; unrelated documentation does not."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from receipt import fingerprint


class SourceFingerprint(unittest.TestCase):
    def test_relocated_sources_are_tracked_without_releasing_for_documents(self):
        previous = Path.cwd()
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            def git(*args):
                subprocess.run(['git', '-C', folder, *args], check=True,
                               stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            def commit():
                git('add', '.')
                git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                    '-c', 'commit.gpgsign=false', 'commit', '-m', 'fixture')
            git('init')
            source = root / 'os/images/appliance.nix'
            source.parent.mkdir(parents=True)
            source.write_text('{ revision = 1; }')
            commit()
            try:
                os.chdir(root)
                baseline = fingerprint()
                (root / 'README.md').write_text('Documentation only')
                commit()
                self.assertEqual(fingerprint(), baseline)
                source.write_text('{ revision = 2; }')
                commit()
                self.assertNotEqual(fingerprint(), baseline)
            finally:
                os.chdir(previous)


if __name__ == '__main__':
    unittest.main()
