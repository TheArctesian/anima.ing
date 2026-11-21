import json
import os
import subprocess
import sys
import time

child = subprocess.Popen([sys.argv[1]], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
assert json.loads(child.stdout.readline())['id'] == 'unlock'
transcript = []
def call(op, args=None):
    child.stdin.write(json.dumps(dict(id=op, op=op, args=args or {})) + '\n')
    child.stdin.flush()
    response = json.loads(child.stdout.readline())
    transcript.append(response)
    return response

assert not call('enroll')['ok'], 'Enrollment requires explicit completed sign-in'
assert call('sign-in')['value']['userCode'] == 'TEST-CODE'
assert call('sign-in-poll')['value']['waiting']
time.sleep(5.1)
assert call('sign-in-poll')['value']['email'] == 'owner@example.invalid'
assert not os.path.exists('/var/lib/anima-daemon/daemon.json'), 'Sign-in must not enroll before consent'
assert call('enroll')['value']['enrolled']
assert json.load(open('/var/lib/anima-daemon/daemon.json')) == dict(synthetic=True)
assert not call('enroll')['ok'], 'Consumed account token must not be reused'
assert call('sign-in')['ok']
assert call('cancel-sign-in')['ok']
assert not call('sign-in-poll')['ok']
selfhost = dict(hub='https://hub.example.invalid', code='anima_enrol_synthetic')
assert not call('enroll-selfhost', selfhost)['ok'], 'Existing ownership must not be replaced'
os.unlink('/var/lib/anima-daemon/daemon.json')
assert call('enroll-selfhost', selfhost)['value']['enrolled']
assert not call('enroll-selfhost', selfhost)['ok'], 'Self-hosted ownership must not be replaced'
child.stdin.close()
assert child.wait(timeout=10) == 0
serialized = json.dumps(transcript)
for secret in ['synthetic-device-secret', 'synthetic-access-secret', 'synthetic-hub-token', 'anima_enrol_synthetic', 'synthetic-link-invite']:
    assert secret not in serialized
assert child.stderr.read() == ''
events = [json.loads(line) for line in open('/run/account-fixture/events')]
queries = [event['data']['query'] for event in events if event['path'] == '/graphql']
assert any('logout' in query for query in queries), 'Temporary hub token must be revoked'
assert not any('createDevice' in query for query in queries), 'Existing matching device must be reused'
assert open('/var/lib/anima-link/invite').read() == 'synthetic-link-invite\n'
assert open('/var/lib/anima-daemon/code').read() == 'anima_enrol_synthetic\n'
assert os.stat('/var/lib/anima-daemon/daemon.json').st_uid != 0, 'Daemon enrollment must run as daemon user'
print('PASS: native TLS device sign-in, consent, Unix hub enrollment, secret isolation, existing device reuse, token revocation and self-hosted ownership')
