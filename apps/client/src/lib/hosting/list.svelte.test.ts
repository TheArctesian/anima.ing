// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, expect, it } from 'vitest';
import { installAnimations } from '$lib/forms/animations.fixture';
import { fakeHosting, type FakeHosting } from './hosting.fixture';
import HostingList from './HostingList.svelte';
import { HostingStore } from './store.svelte';

let app: ReturnType<typeof mount> | undefined;
let calls: FakeHosting;

beforeEach(() => {
  installAnimations();
  calls = fakeHosting();
});

afterEach(async () => {
  if (app !== undefined) await unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

async function open(): Promise<HostingStore> {
  const store = new HostingStore(calls);
  app = mount(HostingList, { target: document.body, props: { store } });
  await store.load();
  flushSync();
  return store;
}

const text = (selector: string) => [...document.querySelectorAll(selector)].map((each) => each.textContent.replace(/\s+/g, ' ').trim());

it('HSE-002: reads the containers of each machine and shows one group for each, with Nothing installed for an empty one', async () => {
  await open();
  expect(calls.containersAsked.toSorted()).toEqual(['m-mini', 'm-nas', 'm-pop']);
  expect(text('tr.group th')).toEqual(['mini · Nothing installed', 'nas · 1 item 0 of 1 running', 'pop-os · 2 items 3 of 3 running']);
  expect(text('tbody th[scope="row"]')).toEqual(['samba Not managed example/samba', '▸ immich Not managed Compose project · 2 containers', 'watchtower Not managed containrrr/watchtower']);
});

it('HSE-002: an application opens to its containers', async () => {
  await open();
  const toggle = document.querySelector<HTMLButtonElement>('button.open');
  expect(toggle?.getAttribute('aria-expanded')).toBe('false');
  toggle?.click();
  flushSync();
  expect(toggle?.getAttribute('aria-expanded')).toBe('true');
  expect(text('tr.inner th')).toEqual(['immich-redis Container of immich', 'immich-server Container of immich']);
});

it('HSE-002: the field finds a container on any machine and hides the machines with no match', async () => {
  const store = await open();
  store.search = 'watch';
  flushSync();
  expect(text('tr.group th')).toEqual(['pop-os · 1 item 3 of 3 running']);
});

it('HSE-002: a machine that does not answer keeps its group with the words of the hub, and the others still show', async () => {
  calls.failing['m-nas'] = 'Docker was not read: the device has no SSH credential in Anima.';
  await open();
  expect(text('tr.group th')[1]).toBe('nas Docker was not read: the device has no SSH credential in Anima.');
  expect(text('tbody th[scope="row"]')).toHaveLength(2);
});

it('HOST-021: looks for newer images only when the operator asks, on each machine that answered', async () => {
  calls.failing['m-nas'] = 'No Docker.';
  const store = await open();
  expect(calls.updatesAsked).toEqual([]);
  await store.checkUpdates();
  expect(calls.updatesAsked.toSorted()).toEqual(['m-mini', 'm-pop']);
});
