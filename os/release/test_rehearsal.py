"""A rehearsal binds one disposable key to both generations and the signed archive."""
import base64
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

from receipt import digest, signed_update
from feed import FEED

ROOT = Path(__file__).resolve().parents[1]


class Rehearsal(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        self.source = root / 'source'
        self.source.mkdir()
        self.output = root / 'rehearsal'
        self.output.mkdir()
        self.run_source('git', 'init', '-q')
        old = self.source / 'experiments/anima-os/flake.nix'
        old.parent.mkdir(parents=True)
        old.write_text('# legacy fixture\n')
        self.commit('legacy')
        baseline = self.run_source('git', 'rev-parse', 'HEAD').stdout.strip()
        (self.source / 'os').mkdir()
        shutil.copyfile(ROOT / '.gitignore', self.source / 'os/.gitignore')
        for name in ('release/rehearsal.py', 'release/qualification.py', 'release/receipt.py',
                     'updates/feed.py', 'release/signing/preview-public.pem'):
            target = self.source / 'os' / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, target)
        self.commit('candidate')
        candidate = self.run_source('git', 'rev-parse', 'HEAD').stdout.strip()

        private, public = root / 'fixture-private.pem', self.output / 'fixture-public.pem'
        subprocess.run(['openssl', 'genpkey', '-algorithm', 'RSA', '-pkeyopt', 'rsa_keygen_bits:2048',
                        '-out', private], check=True, capture_output=True)
        subprocess.run(['openssl', 'pkey', '-in', private, '-pubout', '-out', public],
                       check=True, capture_output=True)
        self.baseline = self.image('baseline', baseline, 'a', public)
        self.candidate = self.image('candidate', candidate, 'b', public)
        self.archive = self.sign(self.candidate, private)

    def run_source(self, *args, check=True):
        return subprocess.run(args, cwd=self.source, capture_output=True, text=True, check=check)

    def commit(self, message):
        self.run_source('git', 'add', '.')
        self.run_source('git', '-c', 'commit.gpgsign=false', '-c', 'user.name=Fixture',
                        '-c', 'user.email=fixture@example.invalid', 'commit', '-qm', message)

    def image(self, name, revision, letter, public):
        folder = self.output / name
        folder.mkdir()
        (folder / 'anima-os-x86_64.iso').write_bytes(name.encode())
        system = '/nix/store/' + letter * 32 + '-nixos-system-anima-Fixture'
        (folder / 'system').symlink_to(system)
        (folder / 'build-inputs.json').write_text(json.dumps(dict(
            sourceRevision=revision, fixturePublicKeySha256=digest(public))))
        if name == 'candidate':
            package = folder / 'package'
            package.mkdir()
            for file in ('private-access.test', 'test-tools.json', 'test-tools.nar'):
                (package / file).write_text('synthetic ' + file)
            self.run_source(sys.executable, 'os/release/receipt.py', '--unsigned', '--test-tools', folder)
        else:
            # This receipt was assembled while HEAD was the legacy commit.
            (folder / 'candidate.json').write_text(json.dumps(dict(
                sourceRevision=revision, system=system,
                isoSha256=digest(folder / 'anima-os-x86_64.iso'),
                buildInputsSha256=digest(folder / 'build-inputs.json'))))
        return folder

    def sign(self, folder, private):
        version = '20261126000000'
        archive = folder / 'package' / version / 'system.nar.xz'
        archive.parent.mkdir()
        archive.write_bytes(b'fixture NAR')
        payload = archive.with_name('manifest.json')
        payload.write_text(json.dumps(dict(
            format=1, version=version, arch='x86_64-linux', channel='preview',
            system=str((folder / 'system').resolve()), sha256=digest(archive),
            bytes=archive.stat().st_size, unpackedBytes=100,
            url=FEED.rsplit('/', 1)[0]+'/'+version+'/system.nar.xz')))
        signature = subprocess.check_output(['openssl', 'dgst', '-sha256', '-sign', private, payload])
        (folder / 'package/latest.json').write_text(json.dumps(dict(
            payload=base64.b64encode(payload.read_bytes()).decode(),
            signature=base64.b64encode(signature).decode())))
        return archive

    def test_rehearsal_refuses_mixups_without_enabling_public_trust(self):
        def preflight():
            return self.run_source(sys.executable, 'os/release/rehearsal.py', self.output, check=False)

        accepted = preflight()
        self.assertEqual(accepted.returncode, 0, accepted.stderr)
        self.assertEqual(accepted.stdout.strip(), str((self.candidate / 'system').resolve()))
        with self.assertRaisesRegex(ValueError, 'signature verification failed'):
            signed_update(self.candidate)
        for path, replacement, reason in [
            (self.output / 'fixture-public.pem', b'other key', 'do not share the recorded rehearsal key'),
            (self.baseline / 'anima-os-x86_64.iso', b'other ISO', 'Candidate ISO bytes differ'),
            (self.archive, b'X' * self.archive.stat().st_size, 'update bytes differ'),
        ]:
            with self.subTest(reason=reason):
                original = path.read_bytes()
                path.write_bytes(replacement)
                denied = preflight()
                self.assertNotEqual(denied.returncode, 0)
                self.assertIn(reason, denied.stderr)
                path.write_bytes(original)
