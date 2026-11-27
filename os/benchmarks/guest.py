"""Read-only guest samples for the disposable ISO comparison."""
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import time


def memory(mode):
    processes = {}
    for path in Path('/proc').glob('[0-9]*'):
        try:
            status = dict(line.split(':', 1) for line in (path / 'status').read_text().splitlines())
            args = (path / 'cmdline').read_bytes().decode(errors='replace').split('\0')
            processes[int(path.name)] = dict(parent=int(status['PPid']), rss_kib=int(status.get('VmRSS', '0 kB').split()[0]), args=args)
        except (OSError, ValueError):
            pass
    roots = {pid for pid, proc in processes.items() if any(
        (arg.endswith('/app.py') if mode == 'installed' else Path(arg).name in ('calamares', '.calamares-wrapped')) for arg in proc['args'])}
    members = set(roots)
    while True:
        expanded = members | {pid for pid, proc in processes.items() if proc['parent'] in members}
        if expanded == members:
            break
        members = expanded
    mem = dict((key, int(value.split()[0])) for key, value in (
        line.split(':', 1) for line in Path('/proc/meminfo').read_text().splitlines()))
    return dict(mem_total_kib=mem['MemTotal'], mem_available_kib=mem['MemAvailable'],
                shell_tree_rss_kib=sum(processes[pid]['rss_kib'] for pid in members) if members else None,
                shell_roots=sorted(roots), shell_process_count=len(members))


def status_latency():
    started = time.monotonic()
    process = subprocess.Popen(['anima-console-privileged'], stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                               env=dict(os.environ, DISPLAY=':0', XAUTHORITY='/home/admin/.Xauthority'))
    try:
        assert json.loads(process.stdout.readline())['ok']
        startup = (time.monotonic() - started) * 1000
        samples = []
        for index in range(10):
            started = time.monotonic()
            process.stdin.write(json.dumps(dict(id=str(index), op='status', args={})) + '\n')
            process.stdin.flush()
            reply = json.loads(process.stdout.readline())
            assert reply['ok'], reply
            samples.append((time.monotonic() - started) * 1000)
        return dict(session_start_ms=startup, status_ms=samples, status_median_ms=statistics.median(samples))
    finally:
        process.stdin.close()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()


mode = sys.argv[1]
time.sleep(20)
samples = []
for _ in range(5):
    samples.append(memory(mode))
    time.sleep(1)
result = dict(mode=mode, idle_samples=samples)
if mode == 'installed':
    try:
        result['session'] = status_latency()
    except Exception as error:
        result['session_unavailable'] = str(error)
print('ANIMA_PERFORMANCE=' + json.dumps(result))
