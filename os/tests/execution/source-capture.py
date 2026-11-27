"""Exercise the installed command boundaries with a paused Btrfs fixture."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
import uuid

if sys.platform != 'linux' or os.geteuid() != 0 or not Path('/.dockerenv').is_file():
    raise SystemExit('Run this check in a disposable Docker container')
os.umask(0o077)
execution = Path(sys.argv[1])
session = execution.with_name('anima-os-session')
tools = Path('/run/current-system/sw/bin')
workspace_root = Path('/persist/anima-workspaces')
assert not tools.exists() and not workspace_root.exists(), 'Expected a fresh fixture store'
for folder in (tools, workspace_root / 'homes', workspace_root / 'locks',
               Path('/run/anima-os'), Path('/run/anima-hosting-pressure'),
               Path('/persist/anima-admission/reservations'), Path('/persist/anima-console-activity')):
    folder.mkdir(parents=True, mode=0o700, exist_ok=True)
boot = Path('/proc/sys/kernel/random/boot_id').read_text().strip()
Path('/run/anima-os/reconciled-boot').write_text(boot)

btrfs = """case "$1:$2" in
  subvolume:create) exec /bin/mkdir -- "$3" ;;
  subvolume:snapshot) exec /bin/cp -a -- "$4" "$5" ;;
  qgroup:limit)
    case "$4" in
      */contexts/*/source)
        : > /run/anima-capture-quotad
        for i in $(/bin/seq 1 300); do
          if test -f /run/anima-capture-resume; then exit 0; fi
          /bin/sleep .05
        done
        exit 3 ;;
    esac
    test -d "$4" ;;
  property:set) test "$6" = true ;;
  subvolume:delete) exec /bin/rm -rf -- "$3" ;;
  *) exit 2 ;;
esac"""
for name, script in {
    'btrfs': btrfs,
    'docker': 'exit 0',
    'systemctl': 'test "$1:$2" = "--no-block:reboot" && : > /run/anima-capture-reboot',
}.items():
    path = tools / name
    path.write_text('#!/bin/sh\n' + script + '\n')
    path.chmod(0o755)
for name in ('ionice', 'renice'):
    (tools / name).symlink_to(shutil.which(name))


def pressure_sample():
    Path('/run/anima-hosting-pressure/clear').write_text(str(int(time.clock_gettime(time.CLOCK_BOOTTIME))))


def frame(payload):
    return (json.dumps(payload) + '\n').encode()


def execute(operation, **args):
    result = subprocess.run([execution], input=frame(dict(operation=operation, **args)),
                            capture_output=True, timeout=10, check=False)
    reply = json.loads(result.stdout)
    assert result.returncode == int(not reply['ok']), (reply, result.stderr)
    return reply


def console(operation, **args):
    command = dict(id='probe', op=operation, args=args)
    reply = subprocess.check_output([session], input=frame(command), timeout=10,
                                    env={'PATH': f'{tools}:/usr/bin:/bin'})
    return next(row for row in map(json.loads, reply.splitlines()) if row['id'] == 'probe')


def prepare():
    pressure_sample()
    workspace = str(uuid.uuid4())
    reply = execute('prepare', workspace=workspace, storage_bytes=1024**3)
    assert reply['ok'] and reply['value']['phase'] == 'prepared', reply
    return workspace


def pause_capture(identity):
    capture = subprocess.Popen([execution], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    capture.stdin.write(frame(dict(operation='capture', **identity)))
    capture.stdin.close()
    deadline = time.monotonic() + 10
    while capture.poll() is None and time.monotonic() < deadline:
        if Path('/run/anima-capture-quotad').is_file():
            return capture
        time.sleep(.02)
    capture.kill()
    capture.wait(timeout=5)
    raise AssertionError(f'The capture did not reach the staged export: {capture.stdout.read()!r}')


def check():
    workspace = prepare()
    home = workspace_root / 'homes' / workspace / 'workspace'
    (home / 'marker').write_text('source from the snapshot')
    identity = dict(workspace=workspace, generation=0, context=str(uuid.uuid4()))
    process = pause_capture(identity)
    try:
        assert prepare() != workspace
        status = execute('context-status', **identity)
        assert status['ok'] and status['value']['phase'] == 'capturing', status
        assert execute('capture', **identity) == status
        release = execute('context-release', confirmed=True, **identity)
        assert not release['ok'] and 'capture is in progress' in release['error'], release
        impact = console('maintenance-impact')
        assert impact['ok'] and impact['value']['sourceInProgress'] is True, impact
        restart = console('restart', confirmed=True)
        assert not restart['ok'] and 'source capture is still running' in restart['error'], restart
        assert not Path('/run/anima-capture-reboot').exists()
        (home / 'marker').write_text('new workspace edit')
    finally:
        Path('/run/anima-capture-resume').touch()
        process.wait(timeout=15)
    result = json.loads(process.stdout.read())
    assert process.returncode == 0 and result['ok'] and result['value']['phase'] == 'captured', result
    exported = workspace_root / 'contexts' / identity['context'] / 'source/marker'
    assert exported.read_text() == 'source from the snapshot'
    assert execute('capture', **identity) == result
    impact = console('maintenance-impact')
    assert impact['ok'] and impact['value']['sourceInProgress'] is False, impact
    released = execute('context-release', confirmed=True, **identity)
    assert released['ok'] and released['value']['phase'] == 'deleted', released
    restart = console('restart', confirmed=True)
    assert restart['ok'] and restart['value']['requested'], restart
    assert Path('/run/anima-capture-reboot').exists()
    print('Source capture preserved admission, snapshot identity and the maintenance fence')


check()
