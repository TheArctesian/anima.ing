"""Serve the appliance smoke image from a loopback-only registry in the guest."""
from contextlib import contextmanager
import gzip
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import tarfile
from threading import Thread


@contextmanager
def registry_image():
    blobs = {}

    def descriptor(media_type, data):
        digest = 'sha256:' + hashlib.sha256(data).hexdigest()
        blobs[digest] = (media_type, data)
        return {'mediaType': media_type, 'digest': digest, 'size': len(data)}

    with tarfile.open('/etc/anima-smoke-image.tar.gz') as archive:
        saved, = json.load(archive.extractfile('manifest.json'))
        config = descriptor('application/vnd.oci.image.config.v1+json',
                            archive.extractfile(saved['Config']).read())
        layers = [descriptor('application/vnd.oci.image.layer.v1.tar+gzip',
                             gzip.compress(archive.extractfile(layer).read(), mtime=0))
                  for layer in saved['Layers']]
    media_type = 'application/vnd.oci.image.manifest.v1+json'
    manifest = descriptor(media_type, json.dumps(
        {'schemaVersion': 2, 'mediaType': media_type, 'config': config, 'layers': layers},
    ).encode())['digest']

    class FixtureRegistry(BaseHTTPRequestHandler):
        def reply(self, body):
            digest = self.path.rsplit('/', 1)[-1]
            item = (('application/json', b'{}') if self.path == '/v2/' else blobs.get(digest))
            if item is None:
                self.send_error(404)
                return
            media, data = item
            self.send_response(200)
            self.send_header('Docker-Distribution-Api-Version', 'registry/2.0')
            self.send_header('Content-Type', media)
            self.send_header('Content-Length', str(len(data)))
            if digest in blobs:
                self.send_header('Docker-Content-Digest', digest)
            self.end_headers()
            if body:
                self.wfile.write(data)

        def do_GET(self):
            self.reply(True)

        def do_HEAD(self):
            self.reply(False)

        def log_message(self, *_):
            pass

    with ThreadingHTTPServer(('127.0.0.1', 0), FixtureRegistry) as registry:
        server = Thread(target=registry.serve_forever, daemon=True)
        server.start()
        try:
            image = f'127.0.0.1:{registry.server_port}/anima-offline-fixture'
            yield image, manifest
        finally:
            registry.shutdown()
            server.join()
