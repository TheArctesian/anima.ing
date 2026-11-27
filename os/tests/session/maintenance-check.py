"""Exercise the real maintenance and workspace owners in a disposable Linux fixture."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from uuid import uuid4


class Session:
    def __init__(self, executable):
        self.child = subprocess.Popen([executable], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        assert json.loads(self.child.stdout.readline())['id'] == 'unlock'

    def request(self, operation, **args):
        frame = dict(id=operation, op=operation, args=args)
        self.child.stdin.write(json.dumps(frame) + '\n')
        self.child.stdin.flush()
        reply = json.loads(self.child.stdout.readline())
        assert reply['id'] == operation, reply
        return reply

    def close(self):
        self.child.stdin.close()
        assert self.child.wait(timeout=10) == 0
        self.child.stdout.close()


def expect_admission(owner, state):
    reply = owner.request('work-admission')
    assert reply['ok'] and reply['value'] == state, reply


def pressure_admission(owner):
    pressure = Path('/run/anima-hosting-pressure')
    pressure.mkdir(mode=0o700)
    clear = pressure / 'clear'
    active = pressure / 'active'
    try:
        clear.touch(mode=0o600)
        clear.write_text(str(int(time.clock_gettime(time.CLOCK_BOOTTIME))))
        expect_admission(owner, 'clear')
        active.touch(mode=0o600)
        expect_admission(owner, 'hosting-pressure')
        active.unlink()
        clear.unlink()
        expect_admission(owner, 'pressure-unavailable')
    finally:
        active.unlink(missing_ok=True)
        clear.unlink(missing_ok=True)
        pressure.rmdir()


def run(session, execution):
    assert os.environ.get('ANIMA_MAINTENANCE_FIXTURE') == '1'
    observation = Path('/run/anima-maintenance-fixture-docker')
    boot = Path('/proc/sys/kernel/random/boot_id').read_text().strip()
    boot_record = Path('/run/anima-os/reconciled-boot')
    observation.write_text('1234567890ab\n')
    owner = Session(session)
    try:
        expect_admission(owner, 'reconciling')
        boot_record.write_text(boot)
        expect_admission(owner, 'pressure-unavailable')
        pending = boot_record.with_name('maintenance.json')
        pending.write_text(json.dumps(dict(schema=1, id=str(uuid4()), operation='restart',
                                           phase='dispatching', admission_closed=True)))
        try:
            expect_admission(owner, 'maintenance')
        finally:
            pending.unlink()
        initial = owner.request('maintenance-impact')['value']
        assert initial['runningDocker'] == 1 and initial['reservations'] == 0, initial
        assert initial['drain'] is None and not initial['ready'], initial
        assert not owner.request('maintenance-drain-begin')['ok']
        hold = owner.request('maintenance-drain-begin', confirmed=True)['value']
        assert owner.request('maintenance-drain-begin', confirmed=True)['value'] == hold
        expect_admission(owner, 'draining')
        denied_power = owner.request('restart', confirmed=True)
        assert not denied_power['ok'] and 'Docker containers are running' in denied_power['error'], denied_power
        assert owner.request('maintenance-status')['value'] is None
    finally:
        owner.close()

    owner = Session(session)
    try:
        impact = owner.request('maintenance-impact')['value']
        assert impact['drain'] == hold and impact['runningDocker'] == 1, impact
        request = dict(operation='prepare', workspace=str(uuid4()), storage_bytes=1 << 30)
        workspace = subprocess.run([execution], input=json.dumps(request) + '\n',
                                   capture_output=True, text=True, check=False, timeout=10)
        result = json.loads(workspace.stdout)
        assert not result['ok'] and 'draining work for maintenance' in result['error'], result
        assert not owner.request('maintenance-drain-cancel', id=str(uuid4()), confirmed=True)['ok']
        assert owner.request('maintenance-impact')['value']['drain'] == hold
        observation.unlink()
        reservation = Path('/persist/anima-admission/reservations') / (str(uuid4()) + '.json')
        reservation.write_text('{}')
        try:
            impact = owner.request('maintenance-impact')['value']
            assert impact['reservations'] == 1 and impact['runningDocker'] == 0 and not impact['ready'], impact
            denied_power = owner.request('restart', confirmed=True)
            assert not denied_power['ok'] and 'reservations are active' in denied_power['error'], denied_power
        finally:
            reservation.unlink()
        assert owner.request('maintenance-drain-cancel', id=hold['id'], confirmed=True)['ok']
        cleared = owner.request('maintenance-impact')['value']
        assert cleared['drain'] is None and cleared['ready'], cleared
        pressure_admission(owner)
    finally:
        owner.close()


if __name__ == '__main__':
    run(*sys.argv[1:])
