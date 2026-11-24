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
        (self.folder / 'release.json').write_text(json.dumps(dict(version=version, system=release['system'], sourceRevision='b'*40, isoSha256=digest(image))))
        (self.folder / 'baseline.json').write_text('{"version":"20250101000000"}')
        self.storage = Storage()
        for name, replacement in [('client', lambda _: self.storage), ('fetch', self.storage.fetch),
                                  ('verify', lambda body: verify(body, public))]:
            patcher = patch.object(channel, name, replacement)
            patcher.start(); self.addCleanup(patcher.stop)

    def test_complete_public_bytes_promote_both_feeds(self):
        channel.publish(self.folder, self.folder / 'credentials')
        self.assertEqual(self.storage.writes, [channel.PREFIX+'latest.json', channel.PREFIX+'latest-iso.json'])

    def test_corrupt_public_iso_keeps_feeds_unchanged(self):
        self.storage.corrupt = True
        with self.assertRaisesRegex(ValueError, 'Public artifact differs'):
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
