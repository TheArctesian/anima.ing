"""Publication must not promote corrupt bytes, stale versions or raced channel heads."""
import base64
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import channel
import receipt
from feed import digest, verify


class Storage:
    def __init__(self):
        self.objects = {}
        self.writes = []
        self.corrupt = False
        self.race = False

    def get_object(self, Bucket, Key):
        if Key not in self.objects:
            raise channel.ClientError({'ResponseMetadata': {'HTTPStatusCode': 404}, 'Error': {'Code': '404'}}, 'GetObject')
        return {'Body': io.BytesIO(self.objects[Key]), 'ETag': str(hash(self.objects[Key]))}

    def head_object(self, Bucket, Key):
        return self.get_object(Bucket, Key)

    def upload_file(self, path, bucket, key, **kwargs):
        self.objects[key] = Path(path).read_bytes()
        if self.corrupt and key.endswith('.iso'):
            self.objects[key] = b'corrupt image'
        if self.race:
            self.objects[channel.PREFIX + 'latest-iso.json'] = json.dumps({'version': '20990101000000'}).encode()

    def put_object(self, Bucket, Key, Body, **kwargs):
        self.writes.append(Key)
        self.objects[Key] = Body

    def fetch(self, url, output, limit):
        payload = self.objects[channel.PREFIX + url.removeprefix(channel.BASE + '/')]
        if len(payload) > limit:
            raise ValueError('Too large')
        output.write(payload)
        return len(payload)


class Publication(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.folder = Path(self.temp.name)
        key = self.folder / 'key.pem'
        public = self.folder / 'public.pem'
        subprocess.run(['openssl', 'genpkey', '-algorithm', 'RSA', '-pkeyopt', 'rsa_keygen_bits:2048', '-out', str(key)], check=True, capture_output=True)
        subprocess.run(['openssl', 'pkey', '-in', str(key), '-pubout', '-out', str(public)], check=True, capture_output=True)
        package = self.folder / 'package'
        version = '20260101000000'
        archive = package / version / 'system.nar.xz'
        archive.parent.mkdir(parents=True)
        archive.write_bytes(b'signed system archive fixture')
        image = self.folder / 'anima-os-x86_64.iso'
        image.write_bytes(b'qualified ISO fixture')
        release = dict(format=1, version=version, arch='x86_64-linux', channel='preview',
                       system='/nix/store/'+'a'*32+'-nixos-system-anima-AnimaOS',
                       sha256=digest(archive), bytes=archive.stat().st_size, unpackedBytes=100,
                       url=channel.BASE+'/'+version+'/system.nar.xz')
        payload = self.folder / 'manifest.json'
        payload.write_text(json.dumps(release))
        signature = subprocess.check_output(['openssl', 'dgst', '-sha256', '-sign', str(key), str(payload)])
        (package / 'latest.json').write_text(json.dumps(dict(payload=base64.b64encode(payload.read_bytes()).decode(), signature=base64.b64encode(signature).decode())))
        inputs = self.folder / 'build-inputs.json'
        inputs.write_text(json.dumps(dict(sourceRevision='b'*40, payloadSha256={})))
        (self.folder / 'release.json').write_text(json.dumps(dict(
            version=version, system=release['system'], sourceRevision='b'*40,
            isoSha256=digest(image), buildInputsSha256=digest(inputs))))
        baseline = self.folder / 'baseline.json'
        baseline_system = '/nix/store/'+'c'*32+'-nixos-system-anima-AnimaOS'
        baseline.write_text(json.dumps(dict(version='20250101000000', system=baseline_system,
                                            sha256='d'*64, isoSha256='e'*64)))
        self.qualify(image, release, baseline)
        self.storage = Storage()
        for name, replacement in [('client', lambda _: self.storage), ('fetch', self.storage.fetch),
                                  ('verify', lambda body: verify(body, public))]:
            patcher = patch.object(channel, name, replacement)
            patcher.start(); self.addCleanup(patcher.stop)
        verifier = patch.object(receipt, 'verify', lambda body: verify(body, public))
        verifier.start()
        self.addCleanup(verifier.stop)

    def qualify(self, image, release, baseline):
        proof = self.folder / 'qualification'
        proof.mkdir()
        previous = json.loads(baseline.read_text())
        reports = [
            ('fresh-install', dict(iso_sha256=digest(image), installed_system=release['system']), [
                'installed-candidate-generation', 'installed-recorded-data-volume',
                'installed-storage-capacity', 'installed-private-ssh-rdp-vnc',
                'installed-podman-to-docker-cutover-and-reboot',
                'installed-hosting-and-application-terminal',
                'installed-hosting-pressure-outage-and-recovery', 'unselected-disk',
            ]),
            ('railpack', dict(iso_sha256=digest(image), installed_system=release['system']), [
                'offline-install', 'installed-candidate-generation',
                'railpack-node-build-export-run', 'railpack-python-build-export-run',
                'railpack-rust-build-export-run', 'railpack-unsupported-source-refusal',
                'clean-shutdown', 'unselected-disk',
            ]),
            ('upgrade', dict(iso_sha256=previous['isoSha256'], baseline_system=previous['system'],
                             baseline_manifest_sha256=digest(baseline),
                             update_system=release['system'], update_sha256=release['sha256']), [
                'installed-baseline-ready-for-upgrade', 'signed-https-download-staging-without-live-activation',
                'updated-system-health-and-persistence', 'private-ssh-rdp-vnc-desktop-reboot-owner-choice',
                'owner-rollback-preserves-data', 'unselected-disk',
            ]),
        ]
        for lane, fields, cases in reports:
            fields['results'] = [dict(case=case, result='Passed') for case in cases]
            (proof / f'{lane}.json').write_text(json.dumps(fields))

    def test_complete_public_bytes_promote_both_feeds(self):
        channel.publish(self.folder, self.folder / 'credentials')
        self.assertEqual(self.storage.writes, [channel.PREFIX+'latest.json', channel.PREFIX+'latest-iso.json'])

    def test_corrupt_public_iso_keeps_feeds_unchanged(self):
        self.storage.corrupt = True
        with self.assertRaisesRegex(ValueError, 'Public artifact differs'):
            channel.publish(self.folder, self.folder / 'credentials')
        self.assertEqual(self.storage.writes, [])

    def test_passing_reports_for_different_bytes_cannot_publish(self):
        cases = [('fresh-install', 'installed_system', '/nix/store/wrong'),
                 ('fresh-install', 'iso_sha256', 'f'*64),
                 ('railpack', 'iso_sha256', 'f'*64),
                 ('upgrade', 'update_sha256', 'f'*64),
                 ('upgrade', 'iso_sha256', 'f'*64),
                 ('upgrade', 'baseline_manifest_sha256', 'f'*64)]
        for lane, field, value in cases:
            with self.subTest(lane=lane, field=field):
                path = self.folder / f'qualification/{lane}.json'
                original = path.read_text()
                report = json.loads(original)
                report[field] = value
                path.write_text(json.dumps(report))
                try:
                    with self.assertRaisesRegex(ValueError, 'report does not match'):
                        channel.publish(self.folder, self.folder / 'credentials')
                    self.assertEqual(self.storage.objects, {})
                finally:
                    path.write_text(original)
                    self.storage.objects.clear()
                    self.storage.writes.clear()

    def test_qualification_requires_cutover_pressure_recovery_and_rollback(self):
        for lane, case in [('fresh-install', 'installed-podman-to-docker-cutover-and-reboot'),
                           ('fresh-install', 'installed-hosting-pressure-outage-and-recovery'),
                           ('upgrade', 'owner-rollback-preserves-data')]:
            with self.subTest(case=case):
                path = self.folder / f'qualification/{lane}.json'
                original = path.read_text()
                report = json.loads(original)
                report['results'] = [entry for entry in report['results'] if entry['case'] != case]
                path.write_text(json.dumps(report))
                try:
                    with self.assertRaisesRegex(ValueError, 'requires every release check'):
                        channel.publish(self.folder, self.folder / 'credentials')
                    self.assertEqual(self.storage.objects, {})
                finally:
                    path.write_text(original)

    def test_qualification_requires_the_railpack_build_and_refusal_cases(self):
        path = self.folder / 'qualification/railpack.json'
        original = path.read_text()
        for case in ('railpack-rust-build-export-run', 'railpack-unsupported-source-refusal'):
            with self.subTest(case=case):
                report = json.loads(original)
                report['results'] = [entry for entry in report['results'] if entry['case'] != case]
                path.write_text(json.dumps(report))
                with self.assertRaisesRegex(ValueError, 'Railpack report requires every release check'):
                    channel.publish(self.folder, self.folder / 'credentials')
                self.assertEqual(self.storage.objects, {})

    def test_changed_generated_inputs_cannot_publish(self):
        (self.folder / 'build-inputs.json').write_text('{"sourceRevision":"' + 'b'*40 + '"}')
        with self.assertRaisesRegex(ValueError, 'Generated build inputs differ'):
            channel.publish(self.folder, self.folder / 'credentials')
        self.assertEqual(self.storage.objects, {})

    def test_changed_same_version_baseline_iso_keeps_feeds_unchanged(self):
        self.storage.objects[channel.PREFIX+'latest-iso.json'] = b'{"version":"20250101000000","sha256":"changed"}'
        with self.assertRaisesRegex(ValueError, 'Channel bytes changed'):
            channel.publish(self.folder, self.folder / 'credentials')
        self.assertEqual(self.storage.writes, [])

    def test_newer_channel_is_never_replaced(self):
        self.storage.objects[channel.PREFIX+'latest-iso.json'] = b'{"version":"20990101000000"}'
        with self.assertRaisesRegex(ValueError, 'regress'):
            channel.publish(self.folder, self.folder / 'credentials')
        self.assertEqual(self.storage.writes, [])

    def test_manual_release_during_upload_keeps_its_pointer(self):
        self.storage.race = True
        with self.assertRaisesRegex(ValueError, 'regress|changed during upload'):
            channel.publish(self.folder, self.folder / 'credentials')
        self.assertEqual(self.storage.writes, [])


if __name__ == '__main__':
    unittest.main()
