"""Qualify lease expiry and watchdog death against real isolated build units."""
from functools import partial
import json
import shlex
import uuid
from build_checks import build_request, offline_source


def leased_request(machine, receipts, name, operation, fields, seconds, assertion, refused=False):
    payload = shlex.quote(json.dumps({'operation': operation, **fields}))
    target = f'{receipts}/{name}.json'
    execute = (f"printf '%s\\n' {payload} | "
               f"jq -c '.lease.until = (now + {seconds} | todateiso8601)' | "
               f'/run/current-system/sw/bin/anima-os-execution > {target}')
    if refused:
        machine.command(f"if {execute}; then exit 1; fi; jq -e '.ok == false' {target}")
    else:
        machine.command(f"{execute} || {{ cat {target}; exit 1; }}; "
                        f"jq -e {shlex.quote('.ok and (' + assertion + ')')} {target}", timeout=90)


def installed_build_leases(machine):
    workspace, context, job = (str(uuid.uuid4()) for _ in range(3))
    receipts = f'/run/anima-build-lease-check-{job}'
    machine.command(f'mkdir -m 700 {receipts}; systemctl is-active --quiet anima-build-leases')
    request = partial(build_request, machine, receipts)
    request('prepare', 'prepare', '.value.phase == "prepared"', workspace=workspace, storage_bytes=1024**3)
    project = f'/persist/anima-workspaces/homes/{workspace}/workspace'
    offline_source(machine, project, job, 'sleep 300')
    request('capture', 'capture', '.value.phase == "captured"', workspace=workspace, generation=0, context=context)
    fields = dict(workspace=workspace, generation=0, context=context, strategy='dockerfile',
                  resources={'memory_bytes': 512*1024**2, 'cpu_millis': 500, 'tasks': 128},
                  lease={'job': job, 'attempt': 1})
    expiry_journey(machine, receipts, request, fields)
    watchdog_journey(machine, receipts, request, dict(fields, lease=dict(fields['lease'], attempt=2)))
    request('context-release', 'context-release', '.value.phase == "deleted"',
            workspace=workspace, generation=0, context=context, confirmed=True)
    request('workspace-release', 'workspace-release', '.value.phase == "deleted"',
            workspace=workspace, generation=0, confirmed=True)


def expiry_journey(machine, receipts, request, fields):
    attempt = str(uuid.uuid4())
    unit = f'anima-build-{attempt}.service'
    leased_request(machine, receipts, 'launch-expiry', 'build', dict(fields, attempt=attempt), 30,
                   '.value.phase == "running"')
    machine.command(f'test "$(systemctl show -p ControlGroup --value {unit})" = '
                    f'"/anima.slice/anima-agents.slice/{unit}"')
    request('active-usage', 'inventory',
            'any(.value.builds.entries[]; '
            f'.attempt == "{attempt}" and (.usage.cpuMicros | type == "number") '
            'and .usage.memoryBytes > 0 and .usage.tasks > 0 and .resources.tasks == 128)',
            usage=True)
    renewal = {'attempt': attempt, 'lease': fields['lease']}
    wrong = dict(renewal, lease=dict(fields['lease'], attempt=2))
    leased_request(machine, receipts, 'wrong-attempt', 'build-renew', wrong, 60, '', refused=True)
    leased_request(machine, receipts, 'renew', 'build-renew', renewal, 60, '.value.phase == "running"')
    machine.command(f'sleep 35; systemctl is-active --quiet {unit}', timeout=45)
    machine.command(f"timeout 70 sh -c 'while systemctl is-active --quiet {unit}; do sleep 1; done'", timeout=80)
    wait_cancelled = (f"until jq -e '.phase == \"cancelled\"' "
                      f'/persist/anima-builds/{attempt}/attempt.json >/dev/null; do sleep 1; done')
    machine.command('timeout 30 sh -c ' + shlex.quote(wait_cancelled), timeout=40)
    request('expired', 'build-status', '.value.attempt.phase == "cancelled" and .value.runtime.MainPID == "0"',
            attempt=attempt)
    request('expired-usage', 'inventory',
            f'any(.value.builds.entries[]; .attempt == "{attempt}" and .phase == "cancelled" '
            'and has("usage") and .usage == null)', usage=True)
    machine.command(f'test ! -e /persist/anima-admission/reservations/{attempt}.json')
    leased_request(machine, receipts, 'late-renewal', 'build-renew', renewal, 60, '', refused=True)
    request('expired-release', 'build-release', '.value.phase == "deleted"', attempt=attempt, confirmed=True)


def watchdog_journey(machine, receipts, request, fields):
    attempt = str(uuid.uuid4())
    unit = f'anima-build-{attempt}.service'
    leased_request(machine, receipts, 'launch-watchdog', 'build', dict(fields, attempt=attempt), 90,
                   '.value.phase == "running"')
    try:
        machine.command('systemctl stop anima-build-leases', timeout=60)
        machine.command(f"timeout 30 sh -c 'while systemctl is-active --quiet {unit}; do sleep 1; done'", timeout=40)
        request('watchdog-reconcile', 'build-reconcile',
                '(.value.phase == "failed" or .value.phase == "cancelled") and .value.result == null', attempt=attempt)
        machine.command(f'test ! -e /persist/anima-admission/reservations/{attempt}.json')
        request('watchdog-release', 'build-release', '.value.phase == "deleted"', attempt=attempt, confirmed=True)
    finally:
        machine.command('systemctl start anima-build-leases; systemctl is-active --quiet anima-build-leases')
