#!/usr/bin/env python3
"""U3 research only: real user services, synthetic executables, no installed Anima state."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time


def command(*args, check=True):
    return subprocess.run(args, check=check, text=True, capture_output=True)


def durable(path, text):
    temporary = path.with_suffix('.next')
    with temporary.open('w') as stream:
        stream.write(text)
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)
    descriptor = os.open(path.parent, os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def ctl(root, action):
    return command('systemctl', '--user', action, root.name + '.service', check=False)


def start(root):
    command('systemd-run', '--user', '--quiet', '--collect',
            '--unit=' + root.name, '--property=Restart=no',
            '/usr/bin/python3', str(root / 'current'), str(root))


def stop(root):
    ctl(root, 'stop')
    for _ in range(100):
        result = command('systemctl', '--user', 'show', root.name + '.service',
                         '--property=LoadState', '--value', check=False)
        if result.stdout.strip() == 'not-found':
            return
        time.sleep(.05)
    raise RuntimeError('transient service did not unload')


def healthy(root, version):
    try:
        stamp = json.loads((root / 'health').read_text())
        return stamp['version'] == version and time.time() - stamp['time'] < .5
    except (FileNotFoundError, json.JSONDecodeError):
        return False


def apply(root, candidate):
    if not (root / 'pending').exists():
        durable(root / 'previous', (root / 'current').read_text())
        durable(root / 'pending', json.dumps({'candidate': candidate}))
        durable(root / 'current', (root / candidate).read_text())
    candidate = json.loads((root / 'pending').read_text())['candidate']
    if candidate not in ('new', 'crash', 'silent'):
        raise ValueError('unknown fixture candidate')
    stop(root)
    for attempt in range(1, 4):
        start(root)
        deadline = time.monotonic() + 2
        while time.monotonic() < deadline:
            if healthy(root, candidate):
                durable(root / 'result', json.dumps({'state': 'updated', 'attempts': attempt}))
                (root / 'pending').unlink()
                return
            time.sleep(.1)
        alive = ctl(root, 'is-active').returncode == 0
        stop(root)
        # An alive but unresponsive program gets one health deadline, not three restarts.
        if alive:
            break
    durable(root / 'current', (root / 'previous').read_text())
    start(root)
    deadline = time.monotonic() + 3
    while not healthy(root, 'old') and time.monotonic() < deadline:
        time.sleep(.1)
    if not healthy(root, 'old'):
        raise RuntimeError('previous version did not recover')
    durable(root / 'result', json.dumps({'state': 'rolled_back', 'attempts': attempt}))
    (root / 'pending').unlink()


def fixture(version):
    if version == 'crash':
        return 'raise SystemExit(42)\n'
    if version == 'silent':
        return 'import time\ntime.sleep(3600)\n'
    return ('import json, time, pathlib, os, sys\n'
            'root = pathlib.Path(sys.argv[1])\n'
            'while True:\n'
            f'    (root / "health.next").write_text(json.dumps({{"version": "{version}", "time": time.time()}}))\n'
            '    os.replace(root / "health.next", root / "health")\n'
            '    time.sleep(.1)\n')


def scenario(candidate, interrupted=False):
    root = Path(tempfile.mkdtemp(prefix='anima-u3-'))
    updater = root.name + '-updater'
    started = time.monotonic()
    try:
        for version in ('old', 'new', 'crash', 'silent'):
            durable(root / version, fixture(version))
        durable(root / 'current', fixture('old'))
        start(root)
        if interrupted:
            # Persisted handoff, then loss of the updater before it can start the candidate.
            durable(root / 'previous', fixture('old'))
            durable(root / 'pending', json.dumps({'candidate': candidate}))
            durable(root / 'current', fixture(candidate))
            stop(root)
        result = command('systemd-run', '--user', '--quiet', '--wait', '--pipe', '--collect',
                         '--unit=' + updater, '--property=RestrictAddressFamilies=AF_UNIX',
                         '/usr/bin/python3', str(Path(__file__).resolve()),
                         'apply', str(root), candidate, check=False)
        if result.returncode:
            raise RuntimeError(result.stderr)
        observed = json.loads((root / 'result').read_text())
        expected = 'updated' if candidate == 'new' else 'rolled_back'
        assert observed['state'] == expected, observed
        assert observed['attempts'] == (3 if candidate == 'crash' else 1), observed
        assert healthy(root, 'new' if candidate == 'new' else 'old')
        assert not (root / 'pending').exists()
        return {'case': candidate, 'interrupted_handoff': interrupted,
                **observed, 'seconds': round(time.monotonic() - started, 2)}
    finally:
        command('systemctl', '--user', 'stop', updater, check=False)
        stop(root)
        shutil.rmtree(root)


if __name__ == '__main__':
    if len(sys.argv) > 1 and sys.argv[1] == 'apply':
        apply(Path(sys.argv[2]), sys.argv[3])
    else:
        for entry in [('new', False), ('crash', False), ('silent', False), ('crash', True)]:
            print(json.dumps(scenario(*entry)), flush=True)
