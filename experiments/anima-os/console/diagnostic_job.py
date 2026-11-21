"""Observe one persistent diagnostic job across local-console retries."""
import json
import subprocess
import time
import uuid


class DiagnosticJob:
    def __init__(self):
        self.request_id = None
        self.operation_id = None
        self.revision = 0

    def read(self, program):
        fresh = self.request_id is None
        if fresh:
            self.request_id = str(uuid.uuid4())
            self.operation_id = None
            self.revision = 0
        deadline = time.monotonic() + 5
        snapshot = self.exchange(program, 'job-start' if fresh else 'job-get', deadline)
        while snapshot['state'] in ('queued', 'running'):
            if time.monotonic() >= deadline:
                raise ValueError('Diagnostics are still running. Retry to reconnect to the same check.')
            time.sleep(min(0.1, max(0, deadline - time.monotonic())))
            snapshot = self.exchange(program, 'job-get', deadline)
        self.request_id = None
        if snapshot['state'] != 'succeeded':
            raise ValueError('The diagnostic check was interrupted or cancelled. Run it again.')
        result = snapshot.get('result')
        states = ('active', 'inactive', 'failed', 'activating', 'deactivating', 'unknown')
        if not isinstance(result, dict) or any(result.get(key) not in states for key in ('monitoring', 'private_link')):
            raise ValueError('The local control service returned an incompatible diagnostic result.')
        return result

    def exchange(self, program, command, deadline):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise ValueError('Diagnostics are still running. Retry to reconnect to the same check.')
        try:
            response = subprocess.run([program, command, self.request_id], capture_output=True,
                                      text=True, timeout=remaining)
        except (OSError, subprocess.TimeoutExpired) as error:
            raise ValueError('Diagnostics are unavailable. Retry to reconnect to the same check.') from error
        try:
            reply = json.loads(response.stdout)
        except (ValueError, TypeError) as error:
            raise ValueError('Diagnostics are unavailable. Retry to reconnect to the same check.') from error
        if (command == 'job-get' and isinstance(reply, dict) and reply.get('version') == 2
                and reply.get('result') == 'error' and reply.get('value') == 'not_found'):
            self.request_id = None
            raise ValueError('This diagnostic check is no longer available. Run it again.')
        if (response.returncode or not isinstance(reply, dict)
                or reply.get('version') != 2 or reply.get('result') != 'operation'):
            raise ValueError('Diagnostics are unavailable. Retry to reconnect to the same check.')
        snapshot = reply.get('value')
        if (not isinstance(snapshot, dict) or snapshot.get('request_id') != self.request_id
                or snapshot.get('state') not in ('queued', 'running', 'succeeded', 'cancelled', 'interrupted')):
            raise ValueError('The local control service returned an incompatible operation.')
        revision = snapshot.get('revision')
        operation_id = snapshot.get('id')
        if (not isinstance(revision, int) or isinstance(revision, bool) or revision < self.revision
                or not isinstance(operation_id, str)
                or (self.operation_id is not None and operation_id != self.operation_id)):
            raise ValueError('The local control service returned an incompatible operation revision.')
        try:
            uuid.UUID(operation_id)
        except ValueError as error:
            raise ValueError('The local control service returned an incompatible operation identity.') from error
        self.operation_id = operation_id
        self.revision = revision
        return snapshot
