"""Summarize raw repeated artifact measurements without asserting causality."""
import argparse
import json
from pathlib import Path
import statistics


def value(record, key):
    if key.endswith('_memory'):
        mode = key.split('_')[0]
        return statistics.median(sample['mem_available_kib'] / 1024 for sample in record[mode]['idle_samples'])
    if key.endswith('_rss'):
        mode = key.split('_')[0]
        samples = [sample['shell_tree_rss_kib'] for sample in record[mode]['idle_samples']]
        return statistics.median(sample / 1024 for sample in samples) if all(sample is not None for sample in samples) else None
    if key.startswith('session_'):
        return record.get('installed', {}).get('session', {}).get(key.removeprefix('session_'))
    return record.get(key)


def cell(samples):
    samples = [sample for sample in samples if sample is not None]
    if not samples:
        return 'Unavailable'
    if all(sample == samples[0] for sample in samples):
        return f'{samples[0]:,.2f}'
    return f'{statistics.median(samples):,.2f} ({min(samples):,.2f}–{max(samples):,.2f})'


def report(folder):
    records = json.loads((folder / 'results.json').read_text())
    metrics = [
        ('ISO bytes', 'iso_bytes'),
        ('Live serial shell observed (s)', 'live_serial_shell_seconds'),
        ('Live visible installer observed (s)', 'live_visible_installer_seconds'),
        ('Live guest MemAvailable (MiB)', 'live_memory'),
        ('Live installer tree RSS sum (MiB)', 'live_rss'),
        ('Installed lock screen observed (s)', 'installed_locked_screen_observed_seconds'),
        ('Installed guest MemAvailable (MiB)', 'installed_memory'),
        ('Installed locked-console tree RSS sum (MiB)', 'installed_rss'),
        ('Privileged session startup (ms)', 'session_session_start_ms'),
        ('Status round-trip median (ms)', 'session_status_median_ms'),
    ]
    lines = ['# Actual Anima OS artifact comparison', '',
             f'Raw evidence: `{folder}`. Each cell is the median of per-boot measurements, with minimum–maximum where values differ.', '',
             '| Metric | Before | After |', '| --- | ---: | ---: |']
    for label, key in metrics:
        columns = [cell([value(record, key) for record in records if record['label'] == group]) for group in ('before', 'after')]
        lines.append(f'| {label} | {columns[0]} | {columns[1]} |')
    lines.extend(['', 'Per-boot memory values are medians of five idle samples. Status values are medians of ten successful protocol requests. Readiness is an observed upper bound including serial login, polling and (installed) OCR, not a paint timestamp. Live Calamares and installed-console measurements describe different software. Installed idle memory is measured with the graphical console locked; dashboard interaction and render latency are unmeasured.', '',
                  'Artifact-wide comparison cannot attribute a change to Rust or the UI. Host desktop and an unrelated Windows workload remained running; file checksums warm the ISO cache. Guest RAM and CPUs are fixed, but host scheduling/cache/thermal effects remain uncontrolled. RSS sums double-count shared pages. No live account, network, agent work or physical hardware was measured.', '',
                  '## Artifact identity', ''])
    for group in ('before', 'after'):
        group_records = [record for record in records if record['label'] == group]
        first = group_records[0]
        lines.append(f"- {group}: `{first['iso']}`; SHA-256 `{first['iso_sha256']}`; {len(group_records)} fresh offline VM samples.")
    incomplete = sorted(path.name for path in folder.glob('*-incomplete-*'))
    if incomplete:
        lines.extend(['', 'Incomplete attempts excluded from statistics: ' + ', '.join(f'`{name}`' for name in incomplete) + '. Their serial logs remain in the evidence directory.'])
    lines.extend(['', f"All unselected sentinel disks unchanged: {all(record['unselected_disk_unchanged'] for record in records)}.", '',
                  'Graphical usability acceptance remains a separate final operator check.'])
    return '\n'.join(lines) + '\n'


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('results', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    text = report(args.results.resolve())
    if args.output:
        args.output.write_text(text)
    else:
        print(text, end='')
