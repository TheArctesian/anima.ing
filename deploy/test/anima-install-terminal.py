"""Drive the installer terminal against fixture programs only."""
import errno
import json
import os
import pty
import select
import sys
import time

request = json.load(sys.stdin)
pid, terminal = pty.fork()
if pid == 0:
    os.execv(sys.argv[1], sys.argv[1:])

output = bytearray()
code_sent = False
answer_sent = False
deadline = time.monotonic() + 10
try:
    while time.monotonic() < deadline:
        if not select.select([terminal], [], [], 0.1)[0]:
            continue
        try:
            chunk = os.read(terminal, 4096)
        except OSError as error:
            if error.errno == errno.EIO:
                break
            raise
        if not chunk:
            break
        output.extend(chunk)
        if not code_sent and b"Paste the install code and press Return:" in output:
            os.write(terminal, (request["code"] + "\n").encode())
            code_sent = True
        if not answer_sent and b"Type yes to enable lingering:" in output:
            os.write(terminal, request["answer"].encode())
            answer_sent = True
    else:
        os.kill(pid, 9)
        raise RuntimeError("installer terminal timed out")
finally:
    os.close(terminal)
    _, status = os.waitpid(pid, 0)
sys.stderr.buffer.write(output)
sys.exit(os.waitstatus_to_exitcode(status))
