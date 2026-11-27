"""Check OCI hashing and disk admission with synthetic systemd and Btrfs commands."""
import datetime as dt
import fcntl
import hashlib
import io
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tarfile
import time
import uuid

if sys.platform != 'linux' or os.geteuid() != 0 or not Path('/.dockerenv').is_file():
    raise SystemExit('Run this check in a disposable Docker container')
os.umask(0o077)

binary = sys.argv[1]
root = Path('/persist/anima-builds')
unit_bin = Path('/run/current-system/sw/bin')
gib = 1024**3
volume = os.statvfs('/persist')
assert volume.f_blocks * volume.f_frsize == 54 * gib, 'Mount /persist with --tmpfs /persist:size=54g'
assert not unit_bin.exists() and not root.exists(), 'Expected a fresh fixture store'
for directory in [root / 'locks', Path('/run/anima-os'),
                  Path('/persist/anima-workspaces/homes'), Path('/persist/anima-workspaces/locks'),
                  Path('/persist/anima-admission/reservations'), unit_bin]:
    directory.mkdir(mode=0o700, parents=True, exist_ok=True)
for name, script in {
    'systemctl': "printf '%s\\n' LoadState=not-found ActiveState=inactive MainPID=0 ControlPID=0 ControlGroup=",
    'btrfs': """case "$1:$2" in
      property:set) test "$6" = true ;;
      subvolume:create) exec /bin/mkdir -- "$3" ;;
      qgroup:limit) test -d "$4" ;;
      *) exit 2 ;;
    esac""",
}.items():
    tool = unit_bin / name
    tool.write_text('#!/bin/sh\n' + script + '\n')
    tool.chmod(0o755)


def invoke(call):
    body = (json.dumps(call) + '\n').encode()
    process = subprocess.run([binary], input=body, capture_output=True, timeout=20, check=False)
    reply = json.loads(process.stdout)
    assert process.returncode == int(not reply['ok']), (reply, process.stderr)
    return reply


def request(op, id, **opts):
    return invoke(dict(operation='build-' + op, attempt=str(id), **opts))


def blob(body, media):
    return {'mediaType': media, 'digest': 'sha256:' + hashlib.sha256(body).hexdigest(), 'size': len(body)}


def archive(work):
    config = b'{"architecture":"amd64","os":"linux","rootfs":{"type":"layers","diff_ids":[]}}'
    cfg = blob(config, 'application/vnd.oci.image.config.v1+json')
    media = 'application/vnd.oci.image.manifest.v1+json'
    manifest = json.dumps(dict(schemaVersion=2,mediaType=media,config=cfg,layers=[])).encode()
    image = blob(manifest, media)
    index = json.dumps(dict(schemaVersion=2,manifests=[image])).encode()
    path = work / 'image.oci'
    with tarfile.open(path, 'w') as tar:
        for name, body in [('oci-layout', b'{"imageLayoutVersion":"1.0.0"}'), ('index.json', index),
                           ('blobs/sha256/' + cfg['digest'][7:], config),
                           ('blobs/sha256/' + image['digest'][7:], manifest)]:
            entry = tarfile.TarInfo(name)
            entry.size = len(body)
            tar.addfile(entry, io.BytesIO(body))
    with path.open('rb+') as data:
        data.truncate(512 * 1024 * 1024)
        digest = 'sha256:' + hashlib.file_digest(data, 'sha256').hexdigest()
    result = dict(schema=1, phase='exported', archiveDigest=digest,
                  bytes=path.stat().st_size, manifestDigest=image['digest'])
    (work / 'result.json').write_text(json.dumps(result))
    return path


def stage(lease_seconds):
    id = uuid.uuid4()
    work = root / str(id) / 'work'
    work.mkdir(parents=True)
    path = archive(work)
    job = str(uuid.uuid4())
    now = dt.datetime.now(dt.timezone.utc)
    grant = dict(job=job, attempt=1, until=(now + dt.timedelta(seconds=lease_seconds)).isoformat())
    uptime = float(Path('/proc/uptime').read_text().split()[0])
    attempt = dict(schema=1, id=str(id), context=str(uuid.uuid4()), workspace=str(uuid.uuid4()),
                   generation=1, resources=dict(memory_bytes=536870912, cpu_millis=500, tasks=128),
                   phase='running', result=None, source_digest='sha256:' + 'a'*64,
                   toolchain={'schema': 1}, strategy='dockerfile',
                   lease=dict(grant=grant, boot=Path('/proc/sys/kernel/random/boot_id').read_text().strip(),
                              deadline_ms=int(uptime*1000) + lease_seconds*1000))
    (work.parent / 'attempt.json').write_text(json.dumps(attempt))
    return id, path, grant


def pause_after_open(id, path):
    proc = subprocess.Popen([binary], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    proc.stdin.write((json.dumps(dict(operation='build-reconcile', attempt=str(id))) + '\n').encode())
    proc.stdin.close()
    until = time.monotonic() + 10
    while time.monotonic() < until and proc.poll() is None:
        for fd in Path(f'/proc/{proc.pid}/fd').glob('*'):
            try:
                if os.readlink(fd) == str(path):
                    os.kill(proc.pid, signal.SIGSTOP)
                    _, status = os.waitpid(proc.pid, os.WUNTRACED)
                    assert os.WIFSTOPPED(status), 'Verification completed before it could be paused'
                    return proc
            except FileNotFoundError:
                pass
        time.sleep(.005)
    raise AssertionError(proc.stdout.read().decode() if proc.poll() is not None else 'archive was not opened')


def check(outcome):
    expires = outcome == 'expired'
    id, path, grant = stage(3 if expires else 45)
    process = pause_after_open(id, path)
    try:
        with open('/run/anima-os/maintenance.lock', 'rb+') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        if expires:
            expiry = dt.datetime.fromisoformat(grant['until'])
            time.sleep(max(0, (expiry - dt.datetime.now(dt.timezone.utc)).total_seconds()))
        else:
            grant['until'] = (dt.datetime.now(dt.timezone.utc) + dt.timedelta(seconds=80)).isoformat()
            renewal = request('renew', id, lease=grant)
            assert renewal['ok'], renewal
            if outcome == 'cancelled':
                result = request('cancel', id)
                assert result['ok'] and result['value']['phase'] == 'cancelled', result
    finally:
        os.kill(process.pid, signal.SIGCONT)
        process.wait(timeout=60)
    reply = json.loads(process.stdout.read())
    if expires:
        assert process.returncode == 1 and not reply['ok'], reply
        assert 'lease expired during artifact verification' in reply['error'], reply
        assert json.loads((path.parent.parent / 'attempt.json').read_text())['result'] is None
    else:
        assert process.returncode == 0 and reply['ok'], reply
        assert reply['value']['phase'] == outcome, reply
        if outcome == 'verified':
            assert reply['value']['result']['integrityVerified'], reply
        else:
            assert reply['value']['result'] is None, reply
    return path


def check_disk_admission(artifact):
    original = json.loads((artifact.parent / 'result.json').read_text())['archiveDigest']
    stage(45)  # A separate writable build keeps its full 16 GiB reservation.
    context = uuid.uuid4()
    folder = Path('/persist/anima-workspaces/contexts') / str(context)
    source = folder / 'source'
    source.mkdir(parents=True)
    (source / 'sentinel').write_text('captured source remains available')
    (folder / 'context.json').write_text(json.dumps(dict(
        schema=1, id=str(context), workspace=str(uuid.uuid4()), generation=1, phase='captured',
        digest='sha256:' + 'b'*64, bytes=33, entries=1, quota_bytes=4*gib)))

    boot = Path('/proc/sys/kernel/random/boot_id').read_text().strip()
    Path('/run/anima-os/reconciled-boot').write_text(boot)
    pressure = Path('/run/anima-hosting-pressure')
    pressure.mkdir(mode=0o700, exist_ok=True)
    (pressure / 'clear').write_text(str(int(time.clock_gettime(time.CLOCK_BOOTTIME))))

    full = invoke(dict(operation='prepare', workspace=str(uuid.uuid4()), storage_bytes=6*gib))
    assert not full['ok'] and 'Hosting reserves the remaining disk space' in full['error'], full
    prepared = invoke(dict(operation='prepare', workspace=str(uuid.uuid4()), storage_bytes=4*gib))
    assert prepared['ok'] and prepared['value']['phase'] == 'prepared', prepared
    blocked = invoke(dict(operation='prepare', workspace=str(uuid.uuid4()), storage_bytes=3*gib))
    assert not blocked['ok'] and 'Hosting reserves the remaining disk space' in blocked['error'], blocked
    with artifact.open('rb') as stream:
        assert 'sha256:' + hashlib.file_digest(stream, 'sha256').hexdigest() == original
    assert (source / 'sentinel').read_text() == 'captured source remains available'


artifact = check('verified')
print('Build verification fixture: verified', flush=True)
check_disk_admission(artifact)
print('Disk admission retained sealed bytes and protected the hosting reserve', flush=True)
for outcome in ('cancelled', 'expired'):
    check(outcome)
    print(f'Build verification fixture: {outcome}', flush=True)
