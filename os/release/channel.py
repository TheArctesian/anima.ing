"""Read and promote the OS preview channel after public byte verification."""
import argparse
import io
import json
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'updates'))
from feed import FEED, digest, fetch, verify
from publish import client, ClientError, TransferConfig
from qualification import qualified_baseline
from receipt import signed_candidate

PREFIX = 'os/preview/x86_64-linux/'
BASE = FEED.rsplit('/', 1)[0]
BUCKET = 'anima-releases'


def public_json(name):
    body = io.BytesIO()
    fetch(BASE + '/' + name, body, 65536)
    return json.loads(body.getvalue())


def baseline(destination):
    signed = public_json('latest.json')
    release = verify(json.dumps(signed).encode())
    iso = public_json('latest-iso.json')
    if iso['version'] != release['version']:
        raise ValueError('The public ISO and update feeds differ; wait for the publisher to finish')
    expected = BASE + '/' + release['version'] + '/anima-os-x86_64.iso'
    if iso['url'] != expected or not 0 < iso['bytes'] <= 32 * 1024**3:
        raise ValueError('Invalid baseline ISO address or size')
    destination.mkdir(parents=True, exist_ok=True)
    path = destination / 'baseline.iso'
    with path.open('wb') as output:
        size = fetch(expected, output, iso['bytes'])
    if size != iso['bytes'] or digest(path) != iso['sha256']:
        raise ValueError('Baseline ISO checksum mismatch')
    (destination / 'baseline.json').write_text(json.dumps(dict(release, isoSha256=iso['sha256'])))


def previous(storage, name):
    try:
        response = storage.get_object(Bucket=BUCKET, Key=PREFIX + name)
        return response['Body'].read(), {'IfMatch': response['ETag']}
    except ClientError as error:
        if error.response['ResponseMetadata']['HTTPStatusCode'] != 404:
            raise
        return None, {'IfNoneMatch': '*'}


def upload(storage, path, key):
    try:
        storage.head_object(Bucket=BUCKET, Key=PREFIX + key)
    except ClientError as error:
        if error.response['ResponseMetadata']['HTTPStatusCode'] != 404:
            raise
        storage.upload_file(str(path), BUCKET, PREFIX + key,
                            ExtraArgs={'CacheControl': 'public, max-age=31536000, immutable'},
                            Config=TransferConfig(multipart_chunksize=64*1024**2, max_concurrency=3))
    # Hash the entire public response without keeping another multi-GB copy.
    import hashlib
    class Hasher:
        def __init__(self):
            self.hash = hashlib.sha256()
        def write(self, chunk):
            self.hash.update(chunk)
    received = Hasher()
    size = fetch(BASE + '/' + key, received, path.stat().st_size)
    if size != path.stat().st_size or received.hash.hexdigest() != digest(path):
        raise ValueError('Public artifact differs from the qualified local artifact: ' + key)


def promotion_conditions(storage, release, iso, baseline):
    conditions = {}
    for name, desired in [('latest.json', release), ('latest-iso.json', iso)]:
        body, condition = previous(storage, name)
        if body:
            old = verify(body) if name == 'latest.json' else json.loads(body)
            if old['version'] not in (baseline['version'], desired['version']):
                raise ValueError('Channel changed since upgrade qualification; refusing to regress or replace it')
            if old['version'] == baseline['version']:
                expected_hash = baseline['sha256' if name == 'latest.json' else 'isoSha256']
                if old.get('sha256') != expected_hash:
                    raise ValueError('Channel bytes changed since upgrade qualification')
            if old['version'] > desired['version'] or (old['version'] == desired['version'] and old != desired):
                raise ValueError('Refusing to regress or replace an existing release')
        conditions[name] = condition
    return conditions


def publish(folder, credentials):
    receipt, release = signed_candidate(folder)
    baseline = qualified_baseline(folder, receipt, release)
    image = folder / 'anima-os-x86_64.iso'
    archive = folder / 'package' / release['version'] / 'system.nar.xz'
    iso = dict(version=release['version'], fileName=image.name, url=BASE+'/'+release['version']+'/'+image.name,
               sha256=receipt['isoSha256'], bytes=image.stat().st_size)
    storage = client(credentials)
    conditions = promotion_conditions(storage, release, iso, baseline)
    for path in (image, archive, folder / 'release.json'):
        upload(storage, path, release['version'] + '/' + path.name)
    # A concurrent manual publisher must not be overwritten after a long upload.
    if conditions != promotion_conditions(storage, release, iso, baseline):
        raise ValueError('Channel changed during upload; retry qualification against the new baseline')
    bodies = {'latest.json': (folder / 'package/latest.json').read_bytes(),
              'latest-iso.json': (json.dumps(iso)+'\n').encode()}
    for name, body in bodies.items():
        storage.put_object(Bucket=BUCKET, Key=PREFIX + name, Body=body,
                           ContentType='application/json', CacheControl='no-store', **conditions[name])
    if verify(json.dumps(public_json('latest.json')).encode()) != release or public_json('latest-iso.json') != iso:
        raise ValueError('Public channel verification failed')
    print(json.dumps({'published': release['version'], 'source': receipt['sourceRevision']}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('action', choices=['baseline', 'publish'])
    parser.add_argument('folder', type=Path)
    parser.add_argument('--credentials', type=Path)
    args = parser.parse_args()
    if args.action == 'baseline':
        baseline(args.folder)
    else:
        if args.credentials is None:
            parser.error('--credentials is required for publication')
        publish(args.folder, args.credentials)
