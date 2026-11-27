"""Qualify committed release inputs and their installed generation."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

VERIFY = Path(__file__).resolve().with_name('generation.py')
BUILD = VERIFY.with_name('build.sh')
WORKLOADS = 'os/console/workloads.js'


class Generation(unittest.TestCase):
    def setUp(self):
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.root = Path(scratch.name)
        self.source = self.root / 'source'
        (self.source / 'os/console').mkdir(parents=True)
        (self.source / 'os/modules').mkdir()
        self.git('init', '-q')
        self.git('config', 'user.email', 'fixture@example.invalid')
        self.git('config', 'user.name', 'Fixture')
        (self.source / 'os/modules/containers.nix').write_text('synthetic containers module\n')
        (self.source / WORKLOADS).write_text('synthetic console asset\n')
        self.git('add', 'os')
        self.git('commit', '-qm', 'synthetic candidate')
        revision = self.git('rev-parse', 'HEAD').stdout

        self.generation = self.root / 'aa-nixos-system-anima'
        self.generation.mkdir()
        self.marker = self.generation / 'anima-hosting-cutover-captured'
        self.marker.write_text('2\n')
        self.console = self.root / 'bb-anima-local-console-source'
        self.console.mkdir()
        (self.console / 'workloads.js').write_bytes((self.source / WORKLOADS).read_bytes())
        provenance = self.root / 'cc-anima-os-source-provenance'
        self.revision = provenance / 'share/anima-build/SOURCE-REVISION'
        self.revision.parent.mkdir(parents=True)
        self.revision.write_bytes(revision)
        tool = self.root / 'nix-store'
        tool.write_text(f"#!/bin/sh\nprintf '%s\\n' '{self.console}' '{provenance}'\n")
        tool.chmod(0o700)

    def git(self, *arguments):
        return subprocess.run(['git', '-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=/dev/null', *arguments], cwd=self.source, check=True, capture_output=True)

    def run_verifier(self):
        return subprocess.run([sys.executable, VERIFY, self.generation],
                              env=dict(os.environ, PATH=f'{self.root}:{os.environ["PATH"]}'),
                              cwd=self.source, capture_output=True, text=True, check=False)

    def test_only_the_exact_candidate_generation_qualifies(self):
        accepted = self.run_verifier()
        self.assertEqual(accepted.returncode, 0, accepted.stderr)
        cases = [
            (self.marker, '1\n', 'does not declare cutover format 2'),
            (self.revision, 'd' * 40, 'source revision differs'),
            (self.console / 'workloads.js', 'old console bytes', 'installed workloads.js differs'),
            (self.source / WORKLOADS, 'working copy only', 'differs from candidate revision'),
        ]
        for path, changed, reason in cases:
            with self.subTest(reason=reason):
                original = path.read_text()
                path.write_text(changed)
                try:
                    denied = self.run_verifier()
                    self.assertNotEqual(denied.returncode, 0)
                    self.assertIn(reason, denied.stderr)
                finally:
                    path.write_text(original)

    def test_working_tree_only_asset_cannot_qualify(self):
        self.git('rm', '--cached', WORKLOADS)
        self.git('commit', '-qm', 'asset only in working tree')
        self.assertTrue((self.source / WORKLOADS).is_file())
        denied = self.run_verifier()
        self.assertNotEqual(denied.returncode, 0)
        self.assertIn('missing from candidate revision', denied.stderr)


    def test_candidate_excludes_local_nix_edits_and_generated_payload(self):
        os_source = self.source / 'os'
        (os_source / 'images').mkdir()
        (os_source / 'images/appliance.nix').write_text('{ revision = 1; }\n')
        (os_source / '.gitignore').write_text('payload/\n')
        assembler = os_source / 'release/assemble.sh'
        assembler.parent.mkdir()
        assembler.write_text("""set -eu
test "$2" = unsigned
cat os/images/appliance.nix > "$1/appliance.nix"
git rev-parse HEAD > "$1/revision"
find os -type f | sort > "$1/assembly-files"
""")
        self.git('add', 'os')
        self.git('commit', '-qm', 'candidate assembly')
        revision = self.git('rev-parse', 'HEAD').stdout
        linked = self.root / 'linked'
        self.git('worktree', 'add', '--detach', str(linked), revision.decode().strip())

        dirty = linked / 'os/images/appliance.nix'
        dirty.write_text('{ revision = 2; }\n')
        (linked / 'os/local.nix').touch()
        (linked / 'os/payload').mkdir()
        (linked / 'os/payload/anima-daemon').touch()

        destination = self.root / 'out'
        result = subprocess.run(['bash', BUILD, '--unsigned', destination], cwd=linked,
                                env=dict(os.environ, RUNNER_TEMP=str(self.root)),
                                capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((destination / 'revision').read_bytes(), revision)
        self.assertEqual((destination / 'appliance.nix').read_text(), '{ revision = 1; }\n')
        inputs = (destination / 'assembly-files').read_text().splitlines()
        self.assertNotIn('os/local.nix', inputs)
        self.assertNotIn('os/payload/anima-daemon', inputs)
        self.assertEqual(dirty.read_text(), '{ revision = 2; }\n')

    def test_generated_input_manifest_binds_payload_bytes_to_revision(self):
        payload = self.source / 'os/payload'
        payload.mkdir()
        revision = self.git('rev-parse', 'HEAD').stdout
        inputs = {
            'anima-daemon': b'daemon bytes',
            'anima-link': b'link bytes',
            'SOURCE-REVISION': revision,
            'SOURCE-SHA256': b'projected source checksums',
            'CARGO-LOCK-SHA256': b'cargo lock checksum',
            'GO-SUM-SHA256': b'go sum checksum',
            'BINARY-SHA256': b'daemon checksum',
            'LINK-BINARY-SHA256': b'link checksum',
        }
        for name, data in inputs.items():
            (payload / name).write_bytes(data)
        manifest = self.root / 'build-inputs.json'
        result = subprocess.run([sys.executable, VERIFY, '--record-inputs', manifest],
                                cwd=self.source, capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(manifest.read_text()), {
            'sourceRevision': revision.decode().strip(),
            'payloadSha256': {name: hashlib.sha256(data).hexdigest()
                              for name, data in inputs.items()},
        })

    def test_unsigned_preflight_requires_the_built_iso_and_source_revision(self):
        harness = self.source / 'os/tests/vm/gui_test.py'
        harness.parent.mkdir(parents=True)
        harness.write_text('raise AssertionError("The fixture must never start a VM")\n')
        self.git('add', 'os/tests/vm')
        self.git('commit', '-qm', 'fixture VM harness')
        output = self.root / 'candidate'
        output.mkdir()
        (output / 'system').symlink_to(self.generation, target_is_directory=True)
        iso = output / 'anima-os-x86_64.iso'
        iso.write_bytes(b'built ISO fixture')
        revision = self.git('rev-parse', 'HEAD').stdout.decode().strip()
        (output / 'build-inputs.json').write_text(json.dumps({'sourceRevision': revision}))
        def receipt(*flags):
            subprocess.run([sys.executable, VERIFY.with_name('receipt.py'), output, '--unsigned', *flags],
                           cwd=self.source, check=True, capture_output=True)

        def preflight(*flags):
            return subprocess.run([sys.executable, VERIFY.with_name('qualification.py'), '--unsigned', output, *flags],
                                  cwd=self.source, capture_output=True, text=True, check=False)

        receipt()
        accepted = preflight()
        self.assertEqual(accepted.returncode, 0, accepted.stderr)
        self.assertEqual(accepted.stdout.strip(), str(self.generation.resolve()))
        missing = preflight('--test-tools')
        self.assertNotEqual(missing.returncode, 0)
        self.assertIn('no recorded private-access test tools', missing.stderr)
        package = output / 'package'
        package.mkdir()
        archive = package / 'test-tools.nar'
        for name in ['private-access.test', 'test-tools.json', archive.name]:
            (package / name).write_text(f'fixture {name}')
        receipt('--test-tools')
        accepted = preflight('--test-tools')
        self.assertEqual(accepted.returncode, 0, accepted.stderr)
        original = archive.read_bytes()
        archive.write_bytes(b'foreign tool archive')
        with self.subTest('different private-access tools'):
            rejected = preflight('--test-tools')
            self.assertNotEqual(rejected.returncode, 0)
            self.assertIn('Private-access test tools differ', rejected.stderr)
        archive.write_bytes(original)
        harness.write_text('raise AssertionError("This working-tree harness should be refused")\n')
        dirty_run = preflight('--lane', 'fresh-install', '--output', self.root / 'run')
        self.assertNotEqual(dirty_run.returncode, 0)
        self.assertIn('Qualification harness differs', dirty_run.stderr)
        self.assertFalse((self.root / 'run').exists())
        iso.write_bytes(b'different ISO fixture')
        with self.subTest('different ISO'):
            rejected = preflight()
            self.assertNotEqual(rejected.returncode, 0)
            self.assertIn('Candidate ISO bytes differ', rejected.stderr)
        iso.write_bytes(b'built ISO fixture')
        self.git('commit', '--allow-empty', '-qm', 'different source revision')
        with self.subTest('different source revision'):
            rejected = preflight()
            self.assertNotEqual(rejected.returncode, 0)
            self.assertIn('Candidate source differs', rejected.stderr)
