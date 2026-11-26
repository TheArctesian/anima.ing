"""Prove the real Linux watcher ends the selected shell after relay death."""
import os
import select
import subprocess
import sys


def relay(application):
    shell = subprocess.Popen(['/bin/sleep', '60'])
    with os.fdopen(os.pidfd_open(shell.pid), 'rb') as identity:
        watch = subprocess.Popen(
            [application, 'console-watch', str(os.getpid())],
            stdin=identity, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            start_new_session=True,
        )
    assert watch.stdout.read(1) == b'\x01', watch.communicate(timeout=1)
    print(shell.pid, flush=True)
    shell.wait(timeout=30)


def test(application):
    other = subprocess.Popen(['/bin/sleep', '60'])
    runner = subprocess.Popen([sys.executable, __file__, application, 'relay'],
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        available = select.poll()
        available.register(runner.stdout, select.POLLIN)
        assert available.poll(5000), 'the real watcher never acknowledged ownership'
        selected = int(runner.stdout.readline())
        with os.fdopen(os.pidfd_open(selected), 'rb') as identity:
            exited = select.poll()
            exited.register(identity, select.POLLIN)
            runner.kill()
            assert runner.wait(timeout=3) < 0
            assert exited.poll(5000), 'the selected shell survived relay death'
            assert other.poll() is None, 'the watcher disturbed a different process'
    finally:
        if runner.poll() is None:
            runner.kill()
            runner.wait(timeout=3)
        other.terminate()
        other.wait(timeout=3)


if __name__ == '__main__':
    if len(sys.argv) == 3:
        relay(sys.argv[1])
    else:
        test(sys.argv[1])
