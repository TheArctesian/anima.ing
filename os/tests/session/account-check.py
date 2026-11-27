"""Exercise consent, ownership and failure through the packaged Rust session."""
import json
from pathlib import Path
import subprocess
import sys
import time

FIXTURE = Path('/run/account-fixture')
DAEMON = Path('/var/lib/anima-daemon')
ENROLLMENT = DAEMON / 'daemon.json'
SELFHOST = dict(hub='https://hub.example.invalid', code='anima_enrol_synthetic')
INSTALL_INVITE = 'anima-invite.1.fixture-device.fixture-hub'
CLAIM = 'A' * 43
RECOVERY = 'B' * 43
INSTALL = dict(code=f'anima-install.1.{CLAIM}.{INSTALL_INVITE}')
SECRET_WORDS = ('synthetic-device-secret', 'synthetic-access-secret', 'synthetic-hub-token',
                'anima_enrol_synthetic', 'synthetic-link-invite', 'synthetic-claim',
                INSTALL_INVITE, CLAIM, RECOVERY)


class Session:
    def __init__(self, program):
        self.child = subprocess.Popen([program], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                      stderr=subprocess.PIPE, text=True)
        assert self.read()['id'] == 'unlock'

    def __enter__(self):
        return self

    def __exit__(self, error, *_):
        if error is not None:
            self.child.kill()
        self.child.stdin.close()
        status = self.child.wait(timeout=10)
        if error is None:
            assert status == 0
            errors = self.child.stderr.read()
            assert all(secret not in errors for secret in SECRET_WORDS)
            assert not errors, errors
        self.child.stdout.close()
        self.child.stderr.close()

    def send(self, op, args=None):
        self.child.stdin.write(json.dumps(dict(id=op, op=op, args=args or {})) + '\n')
        self.child.stdin.flush()

    def read(self):
        line = self.child.stdout.readline()
        assert all(secret not in line for secret in SECRET_WORDS)
        return json.loads(line)

    def call(self, op, args=None):
        self.send(op, args)
        reply = self.read()
        assert reply['id'] == op
        return reply

    def sign_in(self):
        enrolled = ENROLLMENT.exists()
        assert self.call('sign-in')['value']['userCode'] == 'TEST-CODE'
        assert self.call('sign-in-poll')['value']['waiting']
        time.sleep(5.1)
        assert self.call('sign-in-poll')['value']['email'] == 'owner@example.invalid'
        assert ENROLLMENT.exists() == enrolled, 'Sign-in must preserve enrollment state'


def lines(path):
    return path.read_text().splitlines() if path.exists() else []


def graph_requests():
    events = (json.loads(line) for line in lines(FIXTURE / 'events'))
    return [event['data'] for event in events if event['path'] == '/graphql']


def hosted(session):
    assert not session.call('enroll')['ok'], 'Enrollment requires completed sign-in'
    session.sign_in()
    reject = DAEMON / 'fixture-reject'
    reject.touch()
    try:
        assert not session.call('enroll')['ok']
        assert lines(DAEMON / 'fixture-attempts') == ['enrol']
        assert not ENROLLMENT.exists()
        assert lines(FIXTURE / 'monitoring') == []
        assert any('logout' in entry['query'] for entry in graph_requests())
        assert not session.call('enroll')['ok'], 'Consumed access cannot be reused after a failure'
    finally:
        reject.unlink()

    session.sign_in()
    assert session.call('enroll')['value']['enrolled']
    assert json.loads(ENROLLMENT.read_text()) == dict(synthetic=True)
    assert ENROLLMENT.stat().st_uid != 0
    assert lines(FIXTURE / 'monitoring') == ['started']
    requests = graph_requests()
    pages = [entry['variables']['after'] for entry in requests if 'devices(' in entry['query']]
    assert pages == [None, 'next', None, 'next']
    assert not any('createDevice' in entry['query'] for entry in requests)
    assert Path('/var/lib/anima-link/invite').read_text() == 'synthetic-link-invite\n'

    ENROLLMENT.unlink()
    (FIXTURE / 'create-device').touch()
    session.sign_in()
    assert session.call('enroll')['value']['enrolled']
    assert lines(FIXTURE / 'monitoring') == ['started', 'started']
    created = [entry for entry in graph_requests() if 'createDevice' in entry['query']]
    assert len(created) == 1
    session.sign_in()
    previous_requests = lines(FIXTURE / 'events')
    refused = session.call('enroll')
    assert not refused['ok'] and 'already enrolled' in refused['error']
    assert lines(FIXTURE / 'events') == previous_requests
    assert session.call('sign-in')['ok']
    assert session.call('cancel-sign-in')['ok']
    assert not session.call('sign-in-poll')['ok']


def self_hosted(session, program):
    attempts = DAEMON / 'fixture-attempts'
    before = len(lines(attempts))
    refused = session.call('enroll-selfhost', SELFHOST)
    assert not refused['ok'] and 'already enrolled' in refused['error']
    ENROLLMENT.unlink()
    for origin in ('http://hub.example.invalid', 'https://user@hub.example.invalid',
                   'https://hub.example.invalid:8443', 'https://hub.example.invalid/path'):
        assert not session.call('enroll-selfhost', dict(SELFHOST, hub=origin))['ok']
    assert len(lines(attempts)) == before

    hold = DAEMON / 'fixture-hold'
    hold.touch()
    session.send('enroll-selfhost', SELFHOST)
    try:
        entered = DAEMON / 'fixture-entered'
        deadline = time.monotonic() + 8
        while not entered.exists():
            assert time.monotonic() < deadline, 'First session never reached the daemon'
            time.sleep(0.05)
        with Session(program) as other:
            rejected = other.call('enroll-selfhost', SELFHOST)
            assert not rejected['ok'] and 'already running' in rejected['error']
        assert len(lines(attempts)) == before + 1
    finally:
        hold.unlink()
    assert session.read()['value']['enrolled']
    assert not session.call('enroll-selfhost', SELFHOST)['ok']
    assert len(lines(attempts)) == before + 1
    assert lines(FIXTURE / 'monitoring') == ['started', 'started', 'started']
    assert (DAEMON / 'code').read_text() == 'anima_enrol_synthetic\n'


def private_install(session, program):
    link = Path('/var/lib/anima-link')
    identity = link / 'device.json'
    before = lines(FIXTURE / 'events')
    if ENROLLMENT.exists():
        assert 'already enrolled' in session.call('enroll-install', INSTALL)['error']
        ENROLLMENT.unlink()
    assert lines(FIXTURE / 'events') == before
    identity.unlink(missing_ok=True)
    (link / 'invite').unlink(missing_ok=True)
    monitoring = lines(FIXTURE / 'monitoring')
    daemon_attempts = lines(DAEMON / 'fixture-attempts')
    for code in ('invalid', f'anima-install.1.{CLAIM}', f'{INSTALL["code"]}\nextra'):
        assert not session.call('enroll-install', dict(code=code))['ok']
    assert not identity.exists()
    assert lines(FIXTURE / 'events') == before

    refused = session.call('enroll-install', INSTALL)
    assert not refused['ok'] and 'new install code' in refused['error']
    assert identity.exists() and not ENROLLMENT.exists()
    assert (link / 'invite').read_text() == INSTALL_INVITE + '\n'
    original = identity.stat()
    assert lines(FIXTURE / 'monitoring') == monitoring
    assert 'existing link name' in session.call('enroll-install', INSTALL)['error']
    assert (link / 'invite').read_text() == INSTALL_INVITE + '\n'
    recovered = subprocess.run([program, 'enroll-install'], check=True, text=True,
                               input=f'anima-install.1.{RECOVERY}\n', capture_output=True)
    assert all(secret not in recovered.stdout + recovered.stderr for secret in SECRET_WORDS)
    assert ENROLLMENT.exists() and ENROLLMENT.stat().st_uid != 0
    retained = identity.stat()
    assert (retained.st_ino, retained.st_mtime_ns) == (original.st_ino, original.st_mtime_ns)
    assert lines(FIXTURE / 'monitoring') == monitoring + ['started']
    assert lines(DAEMON / 'fixture-attempts') == daemon_attempts + ['enrol']
    claims = [entry['variables']['claim'] for entry in graph_requests()
              if 'claimComputerEnrolment' in entry['query']]
    assert claims == [CLAIM, RECOVERY]


if __name__ == '__main__':
    with Session(sys.argv[1]) as session:
        if '--private-install' not in sys.argv[2:]:
            hosted(session)
            self_hosted(session, sys.argv[1])
        private_install(session, sys.argv[1])
    print('PASS: private install and claim recovery' if '--private-install' in sys.argv[2:]
          else 'PASS: native sign-in, self-hosted monitoring, private install and claim recovery')
