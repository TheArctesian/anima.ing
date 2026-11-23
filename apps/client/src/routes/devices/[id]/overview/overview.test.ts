// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, expect, it, vi } from 'vitest';
import { endTransitionsAtOnce } from '$lib/devices/transitions.fixture';
import type { HubError } from '$lib/hub/hub';
import type { MetricRange } from '$lib/metrics/api';
import { INTEGRATED, NVIDIA, TWO_GPUS, added, point, series } from '$lib/metrics/metrics.fixture';
import { POP_OS } from '$lib/metrics/storage/storage.fixture';
import { manualClock } from '$lib/motion/loading/clock.fixture';
import { threshold } from '$lib/motion/tokens';
import { device } from '../page.fixture.svelte';
import type { DeviceMetrics } from './metrics.svelte';
import OverviewPanel from './OverviewPanel.svelte';
import OverviewSkeleton from './OverviewSkeleton.svelte';
import { checkCounts, daemonNeedLine, hostKeyText, noChartsText, uptimeReadings } from './overview';

vi.mock('motion', async (importOriginal) => ({
  ...(await importOriginal<typeof import('motion')>()),
  animate: (await import('$lib/motion/engine.fixture')).fakeRun,
}));

endTransitionsAtOnce();

let app: ReturnType<typeof mount> | undefined;

afterEach(async () => {
  if (app) await unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

it('D-10: uptime for 24 hours, 7 days and 30 days; no value is "—"', () => {
  expect(uptimeReadings(device({ day: 0.99999, week: 0.5, month: null }))).toEqual([
    { label: '24 hours', text: '99.99%' },
    { label: '7 days', text: '50%' },
    { label: '30 days', text: '—' },
  ]);
});

it('D-10: checks are counted by status, worst first, without empty statuses', () => {
  expect(checkCounts([{ status: 'UP' }, { status: 'DOWN' }, { status: 'UP' }])).toEqual([
    { status: 'DOWN', label: 'Down', count: 1 },
    { status: 'UP', label: 'Up', count: 2 },
  ]);
  expect(checkCounts([])).toEqual([]);
});

it.each([
  [{ enabled: true, hostKey: device().ssh.hostKey }, 'ssh-ed25519 SHA256:abc'],
  [{ enabled: true, hostKey: null }, 'No host key accepted yet'],
  [{ enabled: false, hostKey: null }, 'SSH is off'],
])('D-10: the host key line for %j is %j', (ssh, text) => {
  expect(hostKeyText(ssh)).toBe(text);
});

type Shown = Pick<DeviceMetrics, 'view' | 'span' | 'shownRange' | 'chartShape' | 'readings' | 'battery' | 'error' | 'liveError'>;

const NOW = new Date('2026-11-05T09:05:00Z');
const SPAN = { start: NOW.getTime() - 3_600_000, end: NOW.getTime() };
const LOADED: Shown = {
  view: { metricsSource: 'SSH', gpus: [NVIDIA, INTEGRATED], series: TWO_GPUS, drives: [], readings: null, batteries: [] },
  span: SPAN,
  shownRange: 'HOUR',
  chartShape: null,
  readings: { ...added('2026-11-05T09:04:00Z', []).readings, cpuPercent: 20, memoryPercent: 45, diskPercent: 61 },
  error: null,
  liveError: null,
  battery: null,
};
const WAITING: Shown = { view: null, span: null, shownRange: null, chartShape: null, readings: null, battery: null, error: null, liveError: null };

function show(metrics: Shown, source: 'SSH' | 'NONE' | 'DAEMON' = 'SSH', range: MetricRange = 'HOUR', ssh: Partial<ReturnType<typeof device>['ssh']> = {}) {
  const time = manualClock();
  const calls = { range: vi.fn<(range: MetricRange) => void>(), retry: vi.fn<() => void>(), settings: vi.fn<() => void>() };
  app = mount(OverviewPanel, {
    target: document.body,
    props: {
      device: device({ maintenance: true, metricsSource: source }, ssh),
      checks: [{ status: 'UP' }],
      metrics,
      range,
      now: () => NOW,
      clock: time.clock,
      onRange: calls.range,
      onRetry: calls.retry,
      onSettings: calls.settings,
    },
  });
  flushSync();
  return { time, calls };
}

const words = (node: Element | null | undefined) => (node?.textContent ?? '').replace(/\p{Co}/gu, '').replace(/\s+/g, ' ').trim();
/** Labels hidden from screen readers are the ones faded out. */
const shown = (node: Element) => {
  const copy = node.cloneNode(true) as Element;
  for (const hidden of copy.querySelectorAll('[aria-hidden="true"]')) hidden.remove();
  return words(copy);
};
const button = (name: string) => [...document.querySelectorAll('button')].find((one) => shown(one) === name);
const card = (title: string) => [...document.querySelectorAll('section')].find((one) => words(one.querySelector('h2')) === title);
const figures = (title: string) => [...(card(title)?.querySelectorAll('dt') ?? [])].map((term) => [words(term), words(term.nextElementSibling)]);

it('D-10: the panel shows status, maintenance, uptime, check counts and the key, each in its card', () => {
  show(LOADED);
  expect(figures('Status')).toEqual([['Status', 'Up'], ['Maintenance', 'On']]);
  expect(figures('Uptime')).toEqual([['24 hours', '99.95%'], ['7 days', '99%'], ['30 days', '—']]);
  expect(figures('Checks')).toEqual([['Up', '1']]);
  expect(words(card('SSH host key'))).toContain('ssh-ed25519');
  expect(words(card('SSH host key')?.querySelector('.overview-key'))).toBe('SHA256:abc');
});

it('D-10: Copy puts the fingerprint alone on the clipboard and says so', async () => {
  const writeText = vi.fn<(text: string) => Promise<void>>().mockResolvedValue(undefined);
  vi.stubGlobal('navigator', { ...navigator, clipboard: { writeText } });
  show(LOADED);
  button('Copy')?.click();
  await vi.waitFor(() => {
    expect(words(card('SSH host key')?.querySelector('[role="status"]'))).toBe('Copied.');
  });
  expect(writeText).toHaveBeenCalledExactlyOnceWith('SHA256:abc');
  expect(button('Copied')).toBeDefined();
  vi.unstubAllGlobals();
});

it('D-10: with no key the card says why, and offers no Copy', () => {
  show(LOADED, 'SSH', 'HOUR', { enabled: false, hostKey: null });
  expect(words(card('SSH host key'))).toBe('SSH host key SSH is off');
  expect(button('Copy')).toBeUndefined();
});

it('MET-003: a selected resource has one trend and the comparison still contains every chart', async () => {
  show(LOADED);
  const primary = document.querySelector('[aria-label="Selected resource trend"]');
  expect(primary?.querySelector('h3')?.textContent).toBe('CPU');
  expect(document.querySelectorAll('svg[role="img"]')).toHaveLength(1);
  const memory = document.querySelector<HTMLButtonElement>('button[aria-label^="Memory:"]');
  memory?.click();
  flushSync();
  expect(memory?.getAttribute('aria-pressed')).toBe('true');
  expect(primary?.querySelector('h3')?.textContent).toBe('Memory');
  const comparison = [...document.querySelectorAll('summary')].find((one) => one.textContent === 'All resource charts');
  comparison?.click();
  await vi.waitFor(() => expect(comparison?.parentElement?.querySelectorAll('h3').length).toBeGreaterThan(0));
  const titles = [...(comparison?.parentElement?.querySelectorAll('h3') ?? [])].map((one) => one.textContent);
  expect(titles).toEqual([
    'CPU',
    'Memory',
    'Disk /',
    'Disk /data',
    'Network eth0',
    'Load',
    `GPU 0 ${NVIDIA.name} usage`,
    `GPU 0 ${NVIDIA.name} VRAM`,
    `GPU 1 ${INTEGRATED.name} usage`,
  ]);
  expect(comparison?.parentElement?.querySelectorAll('svg[role="img"]')).toHaveLength(titles.length);
});

it('MET-003: the range control marks the current choice and reports a change', () => {
  const { calls } = show(LOADED, 'SSH', 'DAY');
  const choices = [...document.querySelectorAll('[aria-label="Time range"] button')];
  expect(choices.map((one) => [one.textContent.trim(), one.getAttribute('aria-pressed')])).toEqual([
    ['1 hour', 'false'],
    ['24 hours', 'true'],
    ['7 days', 'false'],
  ]);
  button('7 days')?.click();
  expect(calls.range).toHaveBeenCalledExactlyOnceWith('WEEK');
});

it('D-7: with no metrics source the panel says metrics are off, links to Settings and shows no chart', () => {
  const { calls } = show(WAITING, 'NONE');
  expect(document.body.textContent).toContain('Metrics are off. Install the daemon or turn on SSH metrics in Settings.');
  expect(document.querySelector('[aria-label="Time range"]')).toBeNull();
  expect(document.querySelector('svg:not(.icon)')).toBeNull();
  button('Open Settings')?.click();
  expect(calls.settings).toHaveBeenCalledOnce();
});

it('MET-011: an DAEMON device has metrics: the range control shows and "Metrics are off" does not', () => {
  show(LOADED, 'DAEMON');
  expect(document.querySelector('[aria-label="Time range"]')).not.toBeNull();
  expect(document.body.textContent).not.toContain('Metrics are off');
});

it('MET-029: the Storage section follows Metrics with a row per drive; it is absent when metrics are off', () => {
  show({ ...LOADED, view: { ...LOADED.view, drives: POP_OS } as Shown['view'] });
  const headings = [...document.querySelectorAll('section h2')].map((one) => words(one));
  // R-224
  expect(headings.slice(headings.indexOf('Metrics'), headings.indexOf('Metrics') + 2)).toEqual(['Metrics', 'Storage']);
  expect(words(card('Storage'))).toContain('Not mounted');
  if (app) void unmount(app);
  show(WAITING, 'NONE');
  expect(card('Storage')).toBeUndefined();
});

it('D-3: a slow load shows the selected trend skeleton after threshold-loading, with no line', () => {
  const { time } = show(WAITING);
  expect(document.querySelector('.chart-grid')).toBeNull();
  time.advance(threshold.loading);
  flushSync();
  expect(document.querySelectorAll('.chart-grid > .cell')).toHaveLength(1);
  expect(document.querySelector('polyline')).toBeNull();
});

it('MOT-007, 5.7.3 Loading: the skeleton reserves the legend of the first chart when known', () => {
  const { time } = show({ ...WAITING, chartShape: [2, 1] });
  time.advance(threshold.loading);
  flushSync();
  expect(document.querySelectorAll('.chart-grid > .cell').length).toBe(1);
  expect(document.querySelectorAll('.chart-grid > .cell .legend').length).toBe(1);
});

it('D-7: an error shows the hub message and Retry in the chart area', () => {
  const error: HubError = { code: 'INTERNAL', message: 'metrics are not served' };
  const { calls } = show({ ...WAITING, error });
  expect(document.querySelector('[role="alert"]')?.textContent).toContain('metrics are not served');
  button('Retry')?.click();
  expect(calls.retry).toHaveBeenCalledOnce();
});

it('MET-007: data with no series and no readings says it is waiting for metrics', () => {
  show({ ...LOADED, readings: null, view: { metricsSource: 'SSH', gpus: [], series: [], drives: [], readings: null, batteries: [] } });
  expect(document.querySelector('.waiting')?.textContent).toBe('Waiting for metrics');
  expect(document.querySelector('h3')).toBeNull();
});

it('R-322: with no readings, a daemon that waits in the Keychain gives its reason line in place of the wait', () => {
  expect(noChartsText(false, 'last hour', 'The daemon of nas needs you')).toBe('The daemon of nas needs you');
  expect(noChartsText(true, 'last hour', 'The daemon of nas needs you')).toBe('No samples in the last hour');
  expect(noChartsText(false, 'last hour', null)).toBe('Waiting for metrics');
});

it('R-322: the reason line speaks of this computer on its own page and names the device on any other', () => {
  const mac = { id: 'm1', name: 'studio-mac', daemonNeed: 'KEYCHAIN' as const };
  expect(daemonNeedLine(mac, 'm1')).toContain('The daemon of this computer needs you');
  expect(daemonNeedLine(mac, null)).toBe(
    'The daemon of studio-mac needs you: on that Mac, macOS asks whether it may read its key in the Keychain. Allow it there.',
  );
  expect(daemonNeedLine({ ...mac, daemonNeed: null }, 'm1')).toBeNull();
});

it('MET-007: a range with no samples while the bars have readings says the range is empty, not waiting', () => {
  show({ ...LOADED, shownRange: 'DAY', view: { metricsSource: 'SSH', gpus: [], series: [], drives: [], readings: null, batteries: [] } }, 'SSH', 'DAY');
  expect(document.querySelector('.waiting')?.textContent).toBe('No samples in the last 24 hours');
  expect(document.body.textContent).not.toContain('Waiting for metrics');
});

it.each([
  ['NONE', 0],
  ['SSH', 1],
] as const)('D-9: the Overview skeleton for %s reserves %i trends', (source, trendCount) => {
  app = mount(OverviewSkeleton, { target: document.body, props: { range: 'HOUR', source, now: () => NOW } });
  flushSync();
  expect(document.querySelectorAll('.chart-grid > .cell')).toHaveLength(trendCount);
  expect(document.querySelectorAll('svg[role="img"]')).toHaveLength(0);
});

it('MET-003: while a switch is pending the old chart keeps its own range in the label and its own gap', () => {
  const at = (minute: number) => point(`2026-11-05T09:0${String(minute)}:00Z`, 20);
  // Minutes 0, 1, 4, 5: a three-minute hole that breaks an HOUR line but not a DAY one.
  const cpu = series('CPU_PERCENT', 'cpu', [at(0), at(1), at(4), at(5)]);
  show({ ...LOADED, view: { metricsSource: 'SSH', gpus: [], series: [cpu], drives: [], readings: null, batteries: [] }, shownRange: 'HOUR' }, 'SSH', 'DAY');
  expect(document.querySelector('svg[role="img"]')?.getAttribute('aria-label')).toContain('CPU, last hour');
  expect(document.querySelectorAll('.lines polyline')).toHaveLength(2);
  expect(document.querySelector('[aria-pressed="true"]')?.textContent.trim()).toBe('24 hours');
});
