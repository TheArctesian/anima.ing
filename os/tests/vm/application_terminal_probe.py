"""Exercise the installed application terminal against offline Docker and Compose."""

from contextlib import suppress
import json
import os
from pathlib import Path
import pty
import select
import shutil
import signal
import subprocess
import time
import uuid

from fixture_registry import registry_image


def wait_output(master, marker):
    end = time.monotonic() + 25
    response = bytearray()
    while time.monotonic() < end and len(response) < 16384:
        if select.select([master], [], [], .25)[0]:
            try:
                response.extend(os.read(master, 4096))
            except OSError:
                break
            if marker in response:
                return
    raise AssertionError(f'Expected {marker!r} from the terminal, received {response!r}')


def shell(container):
    pid, master = pty.fork()
    if pid == 0:
        os.execlp('anima-application-terminal', 'anima-application-terminal', 'open', container)
    try:
        wait_output(master, b'$ ')
        os.write(master, b"printf 'FIXTURE_TERMINAL:%s:%s\\n' \"$(id -u)\" \"$(id -g)\"; exit\n")
        wait_output(master, b'FIXTURE_TERMINAL:1010:1011')
        end = time.monotonic() + 15
        while time.monotonic() < end:
            ended, status = os.waitpid(pid, os.WNOHANG)
            if ended:
                assert os.waitstatus_to_exitcode(status) == 0
                return
            time.sleep(.1)
        raise AssertionError('The application terminal survived shell exit')
    finally:
        os.close(master)
        with suppress(ChildProcessError):
            if os.waitpid(pid, os.WNOHANG)[0] == 0:
                os.kill(pid, signal.SIGKILL)
                os.waitpid(pid, 0)


def run():
    # Place the exact digest, final form and running service in a dedicated fixture record.
    os.umask(0o077)
    identity = uuid.uuid4()
    application = Path('/persist/anima-applications') / str(identity)
    files = application / 'files'
    files.mkdir(parents=True)
    with registry_image() as (image, digest):
        reference = f'{image}@{digest}'
        subprocess.run(['docker', 'pull', reference], check=True, timeout=90)
    services = {'web': {'image': reference, 'user': '1010:1011', 'network_mode': 'none',
                        'read_only': True, 'security_opt': ['no-new-privileges:true'], 'cap_drop': ['ALL'],
                        'command': ['/bin/httpd', '-f', '-p', '8080', '-h', '/www']}}
    manifest = json.dumps({'services': services})
    final = files / 'final-form.json'
    final.write_text(manifest)
    (files / 'compose-as-it-came.yaml').write_text(manifest)
    (files / 'environment-of-the-application').write_text('')
    (files / 'digests.json').write_text(json.dumps([{'service': 'web', 'image': reference, 'digest': digest}]))
    plan = ['docker-compose', '--project-name', 'anima-' + identity.hex,
            '--project-directory', str(files), '-f', str(final)]
    try:
        subprocess.run([*plan, 'up', '-d', '--pull', 'never', '--no-build'], check=True, timeout=60)
        identifier = subprocess.check_output([*plan, 'ps', '-q', 'web'], text=True, timeout=20).strip()
        target = json.loads(subprocess.check_output(['anima-application-terminal', 'target', identifier], timeout=20))
        assert target['container'] == identifier and target['application'] == str(identity)
        assert target['service'] == 'web' and target['user'] == '1010:1011', target
        observe = ['docker', 'inspect', '--format', '{{.State.Pid}}', identifier]
        before = subprocess.check_output(observe, timeout=20)
        shell(identifier)
        assert int(before) > 1 and subprocess.check_output(observe, timeout=20) == before
        assert subprocess.check_output(['docker', 'exec', identifier, '/bin/wget', '-qO-',
                                        'http://127.0.0.1:8080'], timeout=20) == b'anima-app-ok\n'
    finally:
        subprocess.run([*plan, 'down'], check=True, timeout=60)
        shutil.rmtree(application)
        subprocess.run(['docker', 'image', 'rm', reference], check=True, timeout=20)


if __name__ == '__main__':
    run()
