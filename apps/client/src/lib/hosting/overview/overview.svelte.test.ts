// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, expect, it } from 'vitest';
import { RoutesStore } from '$lib/application-routes/store.svelte';
import { NOW, ROUTES, fakeCalls } from '$lib/application-routes/tests/routes.fixture';
import { fakeHosting, type FakeHosting } from '../hosting.fixture';
import { HostingStore } from '../store.svelte';
import HostingOverview from './HostingOverview.svelte';

let app: ReturnType<typeof mount> | undefined;
let calls: FakeHosting;
let shown: [string, string][];

beforeEach(() => {
  calls = fakeHosting();
  shown = [];
});

afterEach(async () => {
  if (app !== undefined) await unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

async function overview(routes = ROUTES): Promise<HostingStore> {
  const hosting = new HostingStore(calls);
  const store = new RoutesStore(fakeCalls(routes), false, () => NOW);
  app = mount(HostingOverview, { target: document.body, props: { hosting, routes: store, now: NOW, onshow: (machine: string, row: string) => shown.push([machine, row]) } });
  await Promise.all([hosting.load(), store.load()]);
  flushSync();
  return hosting;
}

const text = (selector: string) => [...document.querySelectorAll(selector)].map((each) => each.textContent.replace(/\s+/g, ' ').trim());

it('HSE-019: each number opens its section with Needs you chosen', async () => {
  await overview();
  expect(text('.figures a')).toEqual(['2 Machines up 1 needs you', '2 Applications running 1 needs you', '3 Routes proven 3 need you']);
  expect([...document.querySelectorAll('.figures a')].map((each) => each.getAttribute('href'))).toEqual(['/hosting/machines', '/hosting?show=needs-you', '/hosting/routes?show=needs-you']);
});

it('HSE-019: each thing that needs you has one line with one action', async () => {
  await overview();
  expect(text('.concerns li').slice(0, 3)).toEqual(['mini Down Open device', 'samba on nas Exited Show', 'tracks.example.com Certificate ends in 9 days Open route']);
  expect(document.querySelector('.concerns a')?.getAttribute('href')).toBe('/devices/m-mini');
  [...document.querySelectorAll<HTMLButtonElement>('.concerns button')].find((each) => each.textContent === 'Show')?.click();
  expect(shown).toEqual([['m-nas', `m-nas/container/${'d'.repeat(64)}`]]);
});

it('HSE-019: with nothing that needs you, the list is one line that says so', async () => {
  calls.machines = () => Promise.resolve({ ok: true, value: [{ id: 'm-pop', name: 'pop-os', state: 'UP' }] });
  await overview([]);
  expect(text('.concerns li')).toEqual(['Nothing needs you.']);
});

it('HSE-019: a machine whose containers did not answer is read again from its line', async () => {
  calls.failing = { 'm-nas': 'SSH refused.' };
  await overview([]);
  expect(text('.concerns li')).toContain('nas Containers not read: SSH refused. Read again');
  calls.failing = {};
  [...document.querySelectorAll<HTMLButtonElement>('.concerns button')].find((each) => each.textContent === 'Read again')?.click();
  await Promise.resolve();
  await new Promise((done) => setTimeout(done, 0));
  flushSync();
  expect(text('.concerns li')).not.toContain('nas Containers not read: SSH refused. Read again');
});
