"""Probe the shipped workspace lifecycle and preview brokers in the offline guest."""

from datetime import datetime, timedelta, timezone
import json
from pathlib import Path
import socket
import subprocess
import sys
import time
import uuid


LIFECYCLE = '/run/anima-workspace/control.sock'
PREVIEW = '/run/anima-execution/preview.sock'
COMMANDS = '/run/anima-execution/daemon.sock'
HTTP = b'GET /anima-fixture HTTP/1.0\r\nHost: localhost\r\n\r\n'


def frame(value):
    return json.dumps(value).encode() + b'\n'


def socket_client(path):
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as stream:
        stream.settimeout(75)
        stream.connect(path)
        stream.sendall(sys.stdin.buffer.read())
        stream.shutdown(socket.SHUT_WR)
        try:
            while chunk := stream.recv(4096):
                sys.stdout.buffer.write(chunk)
        except ConnectionResetError:
            pass  # A refused preview may close with an unread HTTP request.


def broker(path, data):
    # The installed brokers verify the OS peer UID. No enrollment token is needed.
    return subprocess.run(
        ['runuser', '-u', 'anima-daemon', '--', sys.executable, __file__, path],
        input=data, stdout=subprocess.PIPE, check=True, timeout=80,
    ).stdout


def deadline():
    return (datetime.now(timezone.utc) + timedelta(seconds=12)).isoformat()


def deliver(identity, **action):
    command = {**identity, 'request': str(uuid.uuid4()), 'action': action}
    reply = json.loads(broker(LIFECYCLE, frame(
        {'operation': 'deliver', 'command': command, 'valid_until': deadline()},
    )))
    assert reply['ok'], reply
    receipt = reply['value']
    assert receipt['command'] == command, receipt
    return receipt


def lifecycle(identity, phase, **action):
    receipt = deliver(identity, **action)
    assert receipt['failure'] is None, receipt
    assert receipt['result']['phase'] == phase, receipt


def execution(request, body=b''):
    return subprocess.run(
        ['anima-os-execution'], input=frame(request) + body, stdout=subprocess.PIPE,
        check=True, timeout=40,
    ).stdout


def wait_for(observe, description):
    end = time.monotonic() + 90
    while time.monotonic() < end:
        if observe():
            return
        time.sleep(1)
    raise RuntimeError(f'Timed out waiting for {description}')


def docker_exec(container, *args, detached=False):
    options = ['--detach'] if detached else []
    return subprocess.run(['docker', 'exec', *options, '--user=1000:1000', container, *args],
                          capture_output=True, timeout=30)


def check_runner(workspace):
    stream = execution({'operation': 'connect', 'workspace': workspace, 'generation': 1},
                       frame({'id': 1, 'call': 'list'}))
    handshake, *replies = map(json.loads, stream.splitlines())
    assert handshake == {'ok': True, 'stream': 'agent-threads/v1'}, handshake
    assert any(reply.get('frame') == 'done' and reply.get('id') == 1
               and reply.get('result', {}).get('outcome') == 'threads' for reply in replies), replies


def start(identity, marker):
    workspace = identity['workspace']
    print(f'Preparing managed workspace {workspace}', flush=True)
    lifecycle(identity, 'prepared', kind='prepare', storage_bytes=1024**3)
    lifecycle(identity, 'running', kind='start', generation=1,
              memory_bytes=384*1024**2, cpu_millis=250, tasks=128)
    wait_for(lambda: json.loads(execution({'operation': 'status', 'workspace': workspace}))
             ['value']['runnerReady'], 'the packaged runner')
    check_runner(workspace)
    container = f'anima-workspace-{workspace}-1'
    for provider in ('codex', 'claude'):
        version = docker_exec(container, f'/bin/{provider}', '--version')
        version.check_returncode()
        assert version.stdout.strip(), provider
    docker_exec(container, '/bin/bash', '-c', 'printf "%s" "$1" > anima-fixture',
                'bash', marker).check_returncode()
    docker_exec(container, '/bin/python3', '-m', 'http.server', '3000', '--bind', '127.0.0.1',
                '--directory', '/home/agent/workspace', detached=True).check_returncode()
    wait_for(lambda: docker_exec(container, '/bin/curl', '-fsS', '--max-time', '2',
                                'http://127.0.0.1:3000/anima-fixture').stdout == marker.encode(),
             'the workspace-local HTTP fixture')
    ports = subprocess.check_output(['docker', 'port', container], timeout=15)
    assert ports == b'', ports


def preview(binding):
    return broker(PREVIEW, frame({'binding': binding, 'valid_until': deadline()}) + HTTP)


def check_previews(workspaces):
    for identity, marker in workspaces:
        binding = {
            **identity, 'endpoint': str(uuid.uuid4()), 'grant': str(uuid.uuid4()),
            'generation': 1, 'port': 3000, 'expires_at': int(time.time()) + 300,
        }
        foreign = {**binding, 'endpoint': str(uuid.uuid4()), 'principal': str(uuid.uuid4())}
        assert preview(foreign) == b'', 'The wrong principal received preview bytes'
        assert preview({**binding, 'generation': 2}) == b'', 'The wrong generation received bytes'
        handshake, _, response = preview(binding).partition(b'\n')
        assert json.loads(handshake) == {'ok': True, 'stream': 'workspace-preview/v1'}, handshake
        headers, _, body = response.partition(b'\r\n\r\n')
        assert headers.startswith(b'HTTP/1.0 200 ') and body == marker.encode(), response

        closed = json.loads(broker(COMMANDS, frame({
            'operation': 'close-preview', 'endpoint': binding['endpoint'],
            'revision': 1, 'principal': identity['principal'],
        })))
        assert closed['ok'] and closed['value']['revoked'], closed
        assert preview(binding) == b'', 'A revoked preview still served bytes'
        print(f'Workspace {identity["workspace"]} served its own port 3000 and revoked it', flush=True)


def storage_usage(workspace):
    snapshot = json.loads(execution({'operation': 'inventory', 'usage': True}))
    row = next(row for row in snapshot['value']['workspaces']['entries'] if row['workspace'] == workspace)
    assert row['storageBytes'] == 1024**3 and isinstance(row['storageUsageBytes'], int), row
    return row['storageUsageBytes']


def check_storage_usage(workspace):
    subprocess.run(['btrfs', 'filesystem', 'sync', '/persist'], check=True, timeout=30)
    before = storage_usage(workspace)
    container = f'anima-workspace-{workspace}-1'
    docker_exec(container, '/bin/dd', 'if=/dev/urandom', 'of=usage-fixture',
                'bs=1048576', 'count=2').check_returncode()
    subprocess.run(['btrfs', 'filesystem', 'sync', '/persist'], check=True, timeout=30)
    after = storage_usage(workspace)
    assert after >= before + 2*1024**2, (before, after)
    print(f'Workspace storage grew from {before} to {after} accounted bytes', flush=True)
    return after


def workspace_identity():
    return {'workspace': str(uuid.uuid4()), 'principal': str(uuid.uuid4())}


def capacity():
    reply = json.loads(broker(LIFECYCLE, frame({'operation': 'ready'})))
    assert reply['ok'] and reply['value']['available'], reply
    return reply['value']['capacity']


def stop_and_delete(identity, sampled_bytes=None):
    lifecycle(identity, 'stopped', kind='stop', generation=1)
    if sampled_bytes is not None:
        assert storage_usage(identity['workspace']) >= sampled_bytes, 'Stopped workspace lost its storage sample'
    lifecycle(identity, 'deleted', kind='delete', generation=1, confirmed=True)


def check_pressure_outage(workspaces, sampled_bytes):
    from pressure_probe import pressure_outage

    (active, marker), (finished, _) = workspaces
    container = f'anima-workspace-{active["workspace"]}-1'
    waiting = workspace_identity()
    ready = capacity()
    assert ready['storageBytes'] >= 1024**3, ready
    with pressure_outage():
        offered = capacity()
        assert offered == dict(storageBytes=0, memoryBytes=0, cpuMillis=0, tasks=0), offered
        receipt = deliver(waiting, kind='prepare', storage_bytes=1024**3)
        assert receipt['result'] is None, receipt
        assert 'No recent hosting pressure observation' in (receipt['failure'] or ''), receipt
        stop_and_delete(finished)
        response = docker_exec(container, '/bin/curl', '-fsS', '--max-time', '2',
                               'http://127.0.0.1:3000/anima-fixture')
        response.check_returncode()
        assert response.stdout == marker.encode(), response.stdout
        check_runner(active['workspace'])
    restored = capacity()
    assert restored['storageBytes'] >= 1024**3, restored
    lifecycle(waiting, 'prepared', kind='prepare', storage_bytes=1024**3)
    lifecycle(waiting, 'deleted', kind='delete', generation=0, confirmed=True)
    stop_and_delete(active, sampled_bytes)


def run():
    workspaces = [
        (workspace_identity(), f'anima-preview-{index}')
        for index in (1, 2)
    ]
    for identity, marker in workspaces:
        start(identity, marker)
    sampled_workspace = workspaces[0][0]['workspace']
    sampled_bytes = check_storage_usage(sampled_workspace)
    check_previews(workspaces)
    check_pressure_outage(workspaces, sampled_bytes)


def installed_workspaces(machine):
    from vm import run_guest_python
    machine.command("timeout 180 sh -c 'until test -S /run/anima-workspace/control.sock "
                    "&& test -S /run/anima-execution/preview.sock; do sleep 2; done'", timeout=190)
    path = Path(__file__)
    run_guest_python(machine, (path, path.with_name('pressure_probe.py')), 660)


if __name__ == '__main__':
    if len(sys.argv) == 2:
        socket_client(sys.argv[1])
    else:
        run()
