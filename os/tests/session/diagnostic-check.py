"""Lose a native console process after the real controller accepts diagnostics."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time


SHIM = """import json, os, pathlib, subprocess, sys, time
folder = pathlib.Path(os.environ['DIAGNOSTIC_FIXTURE'])
reply = subprocess.check_output(json.loads(os.environ['DIAGNOSTIC_CONTROL']) + sys.argv[1:])
hold = folder / 'hold'
if sys.argv[1] == 'job-start' and hold.exists():
    pending = folder / 'accepted.pending'
    pending.write_bytes(reply)
    pending.replace(folder / 'accepted')
    while hold.exists():
        time.sleep(0.05)
    sys.exit(0)
sys.stdout.buffer.write(reply)
"""


def operations(control):
    reply = json.loads(subprocess.check_output(control + ['job-list']))
    assert reply['version'] == 2 and reply['result'] == 'operations'
    assert reply['value']['next'] is None
    return {job['request_id']: job['id'] for job in reply['value']['records']}


def open_session(program, environment):
    process = subprocess.Popen([program], stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, text=True, env=environment)
    assert json.loads(process.stdout.readline())['id'] == 'unlock'
    process.stdin.write(json.dumps(dict(id='diagnostic', op='diagnostics', args={})) + '\n')
    process.stdin.flush()
    return process


def lose_reply(program, environment, folder):
    hold = folder / 'hold'
    accepted = folder / 'accepted'
    hold.touch()
    with open_session(program, environment) as process:
        try:
            deadline = time.monotonic() + 5
            while not accepted.is_file():
                assert time.monotonic() < deadline, 'The real controller never accepted diagnostics'
                time.sleep(0.05)
            reply = json.loads(accepted.read_text())
            assert reply['version'] == 2 and reply['result'] == 'operation'
            return reply['value']
        finally:
            process.kill()
            process.wait(timeout=5)
            hold.unlink()


def complete(program, environment):
    with open_session(program, environment) as process:
        try:
            reply = json.loads(process.stdout.readline())
            assert reply['id'] == 'diagnostic' and reply['ok'], reply
            process.stdin.close()
            assert process.wait(timeout=10) == 0
            return reply['value']
        finally:
            if process.poll() is None:
                process.kill()


def check(program, control):
    with tempfile.TemporaryDirectory(prefix='anima-diagnostic-') as directory:
        folder = Path(directory)
        shim = folder / 'observe-control'
        shim.write_text(f'#!{sys.executable}\n' + SHIM)
        shim.chmod(0o700)
        environment = dict(os.environ, ANIMA_OS_CONTROL=str(shim),
                           DIAGNOSTIC_CONTROL=json.dumps(control), DIAGNOSTIC_FIXTURE=str(folder))
        before = operations(control)
        accepted = lose_reply(program, environment, folder)
        pending = before | {accepted['request_id']: accepted['id']}
        assert operations(control) == pending
        result = complete(program, environment)
        assert operations(control) == pending, 'Reopening diagnostics started a second controller job'
        assert result['monitoring'] == 'active' and result['private_link'] == 'failed'
        assert 'network' in result and 'root' in result
        complete(program, environment)
        after = operations(control)
        assert len(after) == len(pending) + 1 and pending.items() <= after.items()
        print('PASS: diagnostics kept the accepted job across console restart')


if __name__ == '__main__':
    check(sys.argv[1], sys.argv[2:])
