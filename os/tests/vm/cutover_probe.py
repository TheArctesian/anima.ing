"""Transfer a real rootful Podman volume to the installed Hosting service."""
import hashlib
import json
import os
from pathlib import Path
import pwd
import shutil
import subprocess
import sys
import time
import uuid

from fixture_registry import registry_image

ROOT = Path('/persist/anima-applications')
APPLICATION = '0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8da1'
IMPORT = '0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8da2'
BACKUP = '0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8da3'
PROJECT = 'anima-' + APPLICATION.replace('-', '')
SOURCE = 'anima-cutover-source'
ORIGINAL = 'podman-original'
UPDATED = 'docker-write'


def command(*args, **options):
    return subprocess.run(args, stdout=subprocess.PIPE, check=True, timeout=120, **options).stdout


def podman(*args):
    return command('podman', '--remote=false', *args)


def migrate(operation, *args, **options):
    return command('anima-applications', operation, ROOT, APPLICATION, IMPORT, *args, **options)


def stage_source():
    podman('load', '-i', '/etc/anima-smoke-image.tar.gz')
    podman('volume', 'create', SOURCE)
    identifier = podman('run', '-d', '--pull=never', '--network=none', '--restart=no',
                        '--name', SOURCE, '--volume', SOURCE + ':/state',
                        'anima-preview-smoke:local', '/bin/sleep', '3600').decode().strip()
    podman('exec', identifier, '/bin/sh', '-c', f'printf "%s\n" {ORIGINAL} > /state/marker')
    podman('stop', '--time', '2', identifier)
    source = source_volume()
    backup = ROOT / 'imports' / ('recovery-' + BACKUP)
    stage = ROOT / 'imports' / IMPORT
    (stage / 'binds').mkdir(parents=True)
    shutil.copytree(source, backup)
    shutil.copytree(backup, stage / 'binds/state')
    (stage / 'migration.json').write_text(json.dumps(dict(
        schema=1, application=APPLICATION, importId=IMPORT, backupId=BACKUP,
        podmanUid=0, podmanContainers=[identifier], binds=['state'])))
    receipt = migrate('import-binds')
    assert migrate('import-binds') == receipt, 'Import did not replay the original receipt'


def prepare(reference):
    mapping = Path('/run/anima-cutover-bind-map.json')
    mapping.write_text(json.dumps([{'source': 'legacy-state', 'bind': 'state'}]))
    legacy = {'services': {'web': {
        'image': reference,
        'command': ['/bin/sleep', '3600'],
        'volumes': ['legacy-state:/state'],
        'restart': 'unless-stopped',
        'healthcheck': {'test': ['CMD', '/bin/test', '-f', '/state/marker'], 'interval': '5s'},
    }}, 'volumes': {'legacy-state': None}}
    try:
        return migrate('prepare-compose', mapping, input=json.dumps(legacy).encode())
    finally:
        mapping.unlink()


CLIENT = '''
import json, socket, struct, sys

message, compose = json.load(sys.stdin)
with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
    client.settimeout(125)
    client.connect('/run/anima-applications/control.sock')
    with client.makefile('rb') as stream:
        def send(body):
            client.sendall(struct.pack('>I', len(body)) + body)

        def read():
            header = stream.read(4)
            if len(header) != 4:
                raise ValueError('Hosting closed the fixture request')
            return json.loads(stream.read(struct.unpack('>I', header)[0]))

        send(json.dumps(message).encode())
        result = read()
        if result['outcome'] == 'needs_files':
            send(compose.encode())
            result = read()
        if result['outcome'] == 'ready':
            send(b'{"type":"start"}')
            result = read()
        if result['outcome'] == 'checking_health':
            result = read()
        print(json.dumps(result))
'''


def apply(compose):
    item = {
        'id': str(uuid.uuid4()), 'attempt': 1, 'type': 'application_apply', 'kind': 'host',
        'project': None, 'lease': {'until': '2099-01-01T00:00:00Z'}, 'secrets': ['compose_file'],
        'limits': {'seconds': 110, 'idleSeconds': 0, 'memoryBytes': 0, 'diskBytes': 0,
                   'orphanSeconds': 0},
        'parameters': {'application': APPLICATION, 'fileSha256': hashlib.sha256(compose).hexdigest(),
                       'environmentSha256': None, 'pull': True, 'keep': 1},
    }
    message = {'origin': 'https://fixture.invalid:443', 'device': APPLICATION,
               'command': {'type': 'item', 'json': json.dumps(item)}}
    user = pwd.getpwnam('anima-daemon')
    return json.loads(command(sys.executable, '-c', CLIENT,
                              input=json.dumps([message, compose.decode()]).encode(),
                              cwd='/', user=user.pw_uid, group=user.pw_gid, extra_groups=[]))


def source_volume():
    return Path(podman('volume', 'inspect', '--format', '{{.Mountpoint}}', SOURCE).decode().strip())


def candidate():
    deadline = time.monotonic() + 45
    while time.monotonic() < deadline:
        rows = command('docker', 'ps', '--no-trunc', '-q',
                       '--filter', f'label=com.docker.compose.project={PROJECT}').decode().splitlines()
        if rows:
            assert len(rows) == 1, f'Expected one running migrated Docker container, found {rows}'
            health = command('docker', 'inspect', '--format', '{{.State.Health.Status}}', rows[0])
            if health.strip() == b'healthy':
                return rows[0]
        time.sleep(1)
    raise AssertionError('The migrated Docker container did not become healthy')


def confirm():
    final_form = ROOT / APPLICATION / 'files/final-form.json'
    digest = hashlib.sha256(final_form.read_bytes()).hexdigest()
    receipt = command('anima-applications', 'confirm-cutover', ROOT / 'settings.json',
                      '/etc/anima/docker-client', ROOT, APPLICATION, IMPORT, digest,
                      '--verified-persistent-write')
    confirmation = json.loads(receipt)['confirmation']
    assert confirmation['authoritativeRuntime'] == 'docker', confirmation
    assert confirmation['finalFormSha256'] == digest, confirmation
    assert confirmation['persistentWriteVerified'] is True, confirmation
    assert confirmation['health'] == 'healthy', confirmation


def require_source_stopped():
    state = podman('inspect', '--format', '{{.State.Running}}', SOURCE)
    assert state.strip() == b'false', 'The retained Podman workload is live'
    assert (source_volume() / 'marker').read_text().strip() == ORIGINAL
    backup = ROOT / 'imports' / ('recovery-' + BACKUP) / 'marker'
    assert backup.read_text().strip() == ORIGINAL


def migrate_to_docker():
    stage_source()
    with registry_image() as (image, digest):
        compose = prepare(f'{image}@{digest}')
        held = {'outcome': 'refused', 'cause': 'migration_held'}
        assert (reply := apply(compose)) == held, reply
        assert not (ROOT / APPLICATION / 'files/final-form.json').exists()
        migrate('authorize-cutover', '--source-startup-disabled')
        podman('start', SOURCE)
        try:
            assert (reply := apply(compose)) == held, reply
            assert not (ROOT / APPLICATION / 'files/final-form.json').exists()
        finally:
            podman('stop', '--time', '2', SOURCE)
        assert (reply := apply(compose)) == {'outcome': 'applied', 'health': 'healthy'}, reply
    verify_migrated_write()


def verify_migrated_write():
    container = candidate()
    assert command('docker', 'exec', container, '/bin/cat', '/state/marker').strip() == ORIGINAL.encode()
    command('docker', 'exec', container, '/bin/sh', '-c', f'printf "%s\n" {UPDATED} > /state/marker')
    assert command('docker', 'exec', container, '/bin/cat', '/state/marker').strip() == UPDATED.encode()
    assert (ROOT / APPLICATION / 'binds/state/marker').read_text().strip() == UPDATED
    require_source_stopped()
    confirm()


def verify_restart():
    container = candidate()
    assert command('docker', 'exec', container, '/bin/cat', '/state/marker').strip() == UPDATED.encode()
    require_source_stopped()
    confirm()
    command('docker', 'stop', '--time', '2', container)


if __name__ == '__main__':
    os.umask(0o077)
    {'migrate': migrate_to_docker, 'verify-restart': verify_restart}[sys.argv[1]]()
