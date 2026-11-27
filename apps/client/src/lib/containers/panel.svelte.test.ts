// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { installAnimations } from '$lib/forms/animations.fixture';
import { DEVICE, deviceHosting } from '$lib/hosting/hosting.fixture';
import { HostingStore } from '$lib/hosting/store.svelte';
import { APP_ID, fakeCalls, type FakeCalls } from './containers.fixture';
import ContainersPanel from './ContainersPanel.svelte';
import { StorageStore } from './storage/storage.svelte';

vi.mock('motion', async (importOriginal) => ({
  ...(await importOriginal<typeof import('motion')>()),
  animate: (await import('$lib/motion/engine.fixture')).fakeRun,
}));

let app: ReturnType<typeof mount> | undefined;
let calls: FakeCalls;

beforeEach(() => {
  installAnimations();
  calls = fakeCalls();
});

afterEach(async () => {
  if (app !== undefined) await unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

async function open(): Promise<HostingStore> {
  const hosting = new HostingStore(deviceHosting(calls));
  hosting.only = DEVICE;
  app = mount(ContainersPanel, { target: document.body, props: { hosting, storage: new StorageStore(calls) } });
  await hosting.load();
  hosting.toggle(`${DEVICE.id}/project/ai`);
  hosting.toggle(`${DEVICE.id}/project/tracks`);
  flushSync();
  return hosting;
}

async function settle(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
  flushSync();
}

const button = (label: string) => [...document.querySelectorAll<HTMLButtonElement>('button')].find((each) => (each.getAttribute('aria-label') ?? each.textContent.trim()) === label);
const text = (selector: string) => [...document.querySelectorAll(selector)].map((each) => each.textContent.replace(/\s+/g, ' ').trim());
const rowCount = () => document.querySelectorAll('tbody tr:not(.group)').length;

it('HSE-008 HOST-019: the tab shows the one list narrowed to the device, a row for each Compose project with its state, and its containers below it', async () => {
  await open();
  expect(text('tr.group th')).toEqual(['nas · 3 items Up 3 of 4 running']);
  expect(text('tbody th[scope="row"]')).toEqual([
    '▾ ai Not managed Compose project · 1 container',
    'ollama Container of ai',
    'T ▾ tracks Not managed Compose project · 2 containers',
    'tracks-app-1 Container of tracks',
    'tracks-worker-1 Container of tracks',
    'R recorder Not managed example/recorder:1.0',
  ]);
  expect(document.body.textContent).toContain('Partly running');
  expect(document.querySelector('a[href$="/hosting"]')?.textContent).toBe('View in Hosting');
  expect(document.querySelector('select')).toBeNull();
});

it('HOST-013: the log of a container is read only when the operator asks, with 200 lines', async () => {
  const hosting = await open();
  expect(calls.logsAsked).toEqual([]);
  button('Log of tracks-app-1')?.click();
  await settle();
  expect(calls.logsAsked).toEqual([[APP_ID, 200]]);
  expect(hosting.pane?.store.selected).toBe(APP_ID);
  expect(document.querySelector('.log h3')?.textContent).toBe('Log of tracks-app-1');
  expect([...document.querySelectorAll('.lines li > span')].map((line) => line.textContent)).toEqual(['listening on :3000', '<b>ready</b>']);
  expect(document.querySelector('.lines b')).toBeNull();
  expect(button('Log of tracks-app-1')?.getAttribute('aria-pressed')).toBe('true');
});

it('HOST-001: the operator selects the number of lines, and the log is read again', async () => {
  await open();
  button('Log of tracks-app-1')?.click();
  await settle();
  document.querySelector<HTMLButtonElement>('[role="radio"][data-value="1000"]')?.click();
  await settle();
  expect(calls.logsAsked).toEqual([[APP_ID, 200], [APP_ID, 1000]]);
});

it('HOST-001: a log that was cut says so, and closing the log drops its lines', async () => {
  calls.logAnswer = { lines: [{ at: null, text: 'first' }], cut: true };
  const hosting = await open();
  button('Log of tracks-app-1')?.click();
  await settle();
  expect(document.querySelector('.cut')?.textContent).toContain('the newest lines are missing');
  button('Close the log')?.click();
  flushSync();
  expect(document.querySelector('.log')).toBeNull();
  expect(hosting.pane?.store.log).toBeNull();
});

it('HOST-001: a device the hub cannot read keeps its group with the reason of the hub, and Refresh reads it again', async () => {
  calls.fail = { code: 'CONFLICT', message: 'Docker was not read: the device has no SSH credential in Anima.' };
  await open();
  expect(text('tr.group th')).toEqual(['nas Up Docker was not read: the device has no SSH credential in Anima.']);
  expect(rowCount()).toBe(0);
  calls.fail = null;
  button('Refresh')?.click();
  await settle();
  expect(calls.listsAsked).toBe(2);
  expect(rowCount()).toBe(6);
});

it('HSE-002: a device with no container says Nothing installed, with the catalogue', async () => {
  calls = fakeCalls([]);
  await open();
  expect(text('tr.group th')).toEqual(['nas · Nothing installed Up Install from the catalogue']);
  expect(rowCount()).toBe(0);
});

it('HOST-013: a failed log read keeps the table and offers a retry', async () => {
  await open();
  calls.fail = { code: 'UNAVAILABLE', message: 'Docker was not read: the device cannot be reached over SSH.' };
  button('Log of tracks-app-1')?.click();
  await settle();
  expect(document.querySelector('.log [role="alert"]')?.textContent).toContain('cannot be reached over SSH');
  expect(rowCount()).toBe(6);
});

it('HOST-013: when the selected container is gone after a refresh, its log closes', async () => {
  const hosting = await open();
  button('Log of tracks-app-1')?.click();
  await settle();
  calls.containers = calls.containers.filter((each) => each.id !== APP_ID);
  await hosting.load();
  flushSync();
  expect(hosting.pane).toBeNull();
  expect(document.querySelector('.log')).toBeNull();
});
