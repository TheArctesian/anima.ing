"""Exercise authenticated activity receipts on a disposable Linux fixture."""
from contextlib import contextmanager
import itertools
import json
import os
from pathlib import Path
import pwd
import stat
import subprocess
import sys


@contextmanager
def session(program, uid=None):
    environment = {"PATH": os.environ.get("PATH", os.defpath)}
    if uid is not None:
        environment["SUDO_UID"] = str(uid)
    with subprocess.Popen(
        [program], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, env=environment
    ) as child:
        assert json.loads(child.stdout.readline())["ok"] is True
        requests = itertools.count(1)

        def request(operation, args=None):
            identity = str(next(requests))
            child.stdin.write(json.dumps({"id": identity, "op": operation, "args": args or {}}) + "\n")
            child.stdin.flush()
            reply = json.loads(child.stdout.readline())
            assert reply["id"] == identity, reply
            return reply

        try:
            yield request
        finally:
            child.stdin.close()
            assert child.wait(timeout=10) == 0


def check(program):
    operator = pwd.getpwnam("nobody")
    with session(program, operator.pw_uid) as request:
        forged = {"confirmed": False, "actor": "untrusted-request-actor", "password": "synthetic-input"}
        assert request("maintenance-drain-begin", forged)["ok"] is False
        held = request("maintenance-drain-begin", {"confirmed": True})
        assert held["ok"] is True

    # A new privileged session can read the previous account's results.
    with session(program) as request:
        history = request("activity")["value"]["actions"]
        assert history["complete"] is True
        observed = history["entries"][:2]
        assert [row["action"] for row in observed] == ["Hold new work", "Hold new work"]
        assert [row["account"] for row in observed] == [operator.pw_name] * 2
        assert [row["outcome"] for row in observed] == ["Accepted", "Reported failure"]
        assert all(row["atMs"] > 0 for row in observed)
        assert request("maintenance-drain-cancel", {"confirmed": True, "id": held["value"]["id"]})["ok"] is True

    ledger = Path("/persist/anima-console-activity/actions.json")
    assert ledger.stat().st_uid == 0 and stat.S_IMODE(ledger.stat().st_mode) == 0o600
    text = ledger.read_text()
    assert "untrusted-request-actor" not in text and "synthetic-input" not in text


    # The observational history must not become a maintenance prerequisite.
    ledger.chmod(0o644)
    try:
        with session(program) as request:
            held = request("maintenance-drain-begin", {"confirmed": True})
            assert held["ok"] is True
            assert request("activity")["value"]["actions"] is None
            assert request("maintenance-drain-cancel", {"confirmed": True, "id": held["value"]["id"]})["ok"] is True
    finally:
        ledger.chmod(0o600)
    assert ledger.read_text() == text


if __name__ == "__main__":
    if os.environ.get("ANIMA_ACTIVITY_FIXTURE") != "1":
        raise SystemExit("Run this check only in its disposable fixture.")
    check(sys.argv[1])
