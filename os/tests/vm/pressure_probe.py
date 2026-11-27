"""Observe a pressure-controller outage while an installed hosting fixture serves."""
from contextlib import contextmanager
import json
import subprocess
import time


SERVICE = 'anima-hosting-pressure.service'


def command(*args, **options):
    return subprocess.check_output(args, timeout=30, **options)


def admission():
    frames = command('anima-console-privileged',
                     input=b'{"id":"admission","op":"work-admission","args":{}}\n')
    reply = next(frame for frame in map(json.loads, frames.splitlines())
                 if frame.get('id') == 'admission')
    assert reply['ok'], reply
    return reply['value']


def wait_for_admission(expected):
    end = time.monotonic() + 35
    observed = None
    while time.monotonic() < end:
        observed = admission()
        if observed == expected:
            return
        time.sleep(1)
    raise AssertionError(f'Expected work admission {expected}, last observed {observed}')


def hosted_pid(container):
    pid = int(command('docker', 'inspect', '--format', '{{.State.Pid}}', container))
    assert pid > 1, 'Hosted application has no running process'
    return pid


def verify_http(container):
    body = command('docker', 'exec', container, '/bin/wget', '-qO-',
                   'http://127.0.0.1:8080')
    assert body == b'anima-app-ok\n', body


@contextmanager
def pressure_outage():
    container = command(
        'docker', 'run', '-d', '--name', 'anima-pressure-hosted-fixture',
        '--pull=never', '--network=none', '--read-only', '--memory=64m',
        '--cap-drop=ALL', '--security-opt=no-new-privileges', 'anima-preview-smoke:local',
        '/bin/httpd', '-f', '-p', '8080', '-h', '/www',
    ).decode().strip()
    try:
        command('systemctl', 'is-active', SERVICE)
        wait_for_admission('clear')
        before = hosted_pid(container)
        command('systemctl', 'stop', SERVICE)
        try:
            wait_for_admission('pressure-unavailable')
            yield
            assert hosted_pid(container) == before, 'The hosted process restarted during the outage'
            verify_http(container)
        finally:
            command('systemctl', 'start', SERVICE)
        wait_for_admission('clear')
        assert hosted_pid(container) == before, 'The hosted process restarted during recovery'
        verify_http(container)
    finally:
        command('docker', 'rm', '-f', container)
