// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
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
  vi.useRealTimers();
});

async function open(hosting = calls): Promise<HostingStore> {
  const store = new HostingStore(hosting);
  app = mount(HostingList, { target: document.body, props: { store } });
  await store.load();
  flushSync();
  return store;
}

const text = (selector: string) => [...document.querySelectorAll(selector)].map((each) => each.textContent.replace(/\s+/g, ' ').trim());

it('HSE-002: reads the containers of each machine and shows one group for each with the status of the machine, and Nothing installed with the catalogue for an empty one', async () => {
  await open();
  expect(calls.containersAsked.toSorted()).toEqual(['m-mini', 'm-nas', 'm-pop']);
  expect(text('tr.group th')).toEqual(['mini · Nothing installed Down Install from the catalogue', 'nas · 1 item Up 0 of 1 running', 'pop-os · 2 items Up 3 of 3 running']);
  expect(text('tbody th[scope="row"]')).toEqual(['samba Not managed example/samba', '▸ immich Not managed Compose project · 2 containers', 'watchtower Not managed containrrr/watchtower']);
});

it('HSE-002: an application opens to its containers', async () => {
  await open();
  const toggle = document.querySelector<HTMLButtonElement>('button.open');
  expect(toggle?.getAttribute('aria-expanded')).toBe('false');
  toggle?.click();
  flushSync();
  expect(toggle?.getAttribute('aria-expanded')).toBe('true');
  expect(text('tr.inner:not(.started) th')).toEqual(['immich-redis Container of immich', 'immich-server Container of immich']);
});

it('HSE-002: the field finds a container on any machine and hides the machines with no match', async () => {
  const store = await open();
  store.search = 'watch';
  flushSync();
  expect(text('tr.group th')).toEqual(['pop-os · 1 item Up 3 of 3 running']);
});

it('HSE-002: a machine that does not answer keeps its group with the words of the hub, and the others still show', async () => {
  calls.failing['m-nas'] = 'Docker was not read: the device has no SSH credential in Anima.';
  await open();
  expect(text('tr.group th')[1]).toBe('nas Up Docker was not read: the device has no SSH credential in Anima.');
  expect(text('tbody th[scope="row"]')).toHaveLength(2);
});

it('HOST-021: looks for newer images only when the operator asks, on each machine that answered', async () => {
  calls.failing['m-nas'] = 'No Docker.';
  const store = await open();
  expect(calls.updatesAsked).toEqual([]);
  await store.checkUpdates();
  expect(calls.updatesAsked.toSorted()).toEqual(['m-mini', 'm-pop']);
});

it('HOST-001 HOST-017: a container on any machine opens its log or its details below the list, read from its machine', async () => {
  await open();
  const press = (label: string) => [...document.querySelectorAll<HTMLButtonElement>('button')].find((each) => each.getAttribute('aria-label') === label)?.click();
  press('Log of watchtower');
  await Promise.resolve();
  await Promise.resolve();
  flushSync();
  expect(calls.logMachines).toEqual(['m-pop']);
  expect(calls.logs.logsAsked).toEqual([['c'.repeat(64), 200]]);
  expect(document.querySelector('.log h3')?.textContent).toBe('Log of watchtower');
  press('Details of samba');
  await Promise.resolve();
  await Promise.resolve();
  flushSync();
  expect(calls.logMachines).toEqual(['m-pop', 'm-nas']);
  expect(document.querySelector('.log')).toBeNull();
  expect(document.querySelector('.detail h3')?.textContent).toBe('Details of samba');
});

it('HOST-034: the choice Containers lists each container of each machine, and a machine that did not answer stays with its reason', async () => {
  calls.failing['m-nas'] = 'The device has no SSH credential.';
  const store = await open();
  store.view = 'containers';
  flushSync();
  expect(text('tbody th[scope="row"]')).toEqual(['immich-redis Not managed example/tracks:0.36 · Container of immich', 'immich-server Not managed example/tracks:0.36 · Container of immich', 'watchtower Not managed containrrr/watchtower']);
  expect(text('tr.group th')).toContain('nas Up The device has no SSH credential.');
  expect(document.querySelectorAll('button[aria-label^="Log of"]')).toHaveLength(3);
});

it('HOST-030: an open project says the folder and the files that Compose started it from, as Docker says', async () => {
  const store = await open();
  store.toggle('m-pop/project/immich');
  flushSync();
  expect(text('tr.started th')).toEqual(['Started by Compose in /srv/immich from /srv/immich/compose.yaml, /srv/immich/compose.override.yaml']);
  store.view = 'containers';
  flushSync();
  expect(document.querySelector('tr.started')).toBeNull();
});


it('queues a confirmed container action for the exact device and refreshes only after completion', async () => {
  const queue = vi.fn(calls.actions.queue);
  const status = vi.fn(calls.actions.status);
  await open({
    ...calls,
    containers: async (device) => {
      const result = await calls.containers(device);
      return result.ok && device === 'm-pop'
        ? { ok: true, value: result.value.map((container) => ({ ...container, actionsAvailable: true })) }
        : result;
    },
    actions: { queue, status },
  });
  vi.useFakeTimers();
  const button = (label: string) => [...document.querySelectorAll<HTMLButtonElement>('button')].find((each) =>
    each.getAttribute('aria-label') === label || each.textContent?.trim() === label);
  expect(button('Actions for samba')).toBeUndefined();
  button('Actions for watchtower')?.click();
  flushSync();
  expect(document.querySelector('[role="dialog"]')?.textContent).toContain('watchtower on pop-os');
  const action = document.querySelector<HTMLSelectElement>('[role="dialog"] select');
  if (action === null) throw new Error('The action selector did not open.');
  action.value = 'RESTART';
  action.dispatchEvent(new Event('change', { bubbles: true }));
  flushSync();
  button('Queue restart')?.click();
  await Promise.resolve();
  flushSync();
  expect(queue).toHaveBeenCalledExactlyOnceWith('m-pop', 'c'.repeat(64), 'RESTART');
  expect(document.querySelector('[role="status"]')?.textContent).toContain('Queued');
  expect(calls.containersAsked.filter((id) => id === 'm-pop')).toHaveLength(1);

  await vi.advanceTimersByTimeAsync(2500);
  flushSync();
  expect(status).toHaveBeenCalledExactlyOnceWith('action-1');
  expect(document.querySelector('[role="status"]')?.textContent).toContain('Completed.');
  expect(calls.containersAsked.filter((id) => id === 'm-pop')).toHaveLength(2);
});

it('marks a project only while each observed member has the same managed application ID', async () => {
  const id = '0190a000-0000-7000-8002-0000000000ab';
  let managedMembers = 1;
  const store = await open({
    ...calls,
    containers: async (device) => {
      const result = await calls.containers(device);
      return result.ok && device === 'm-pop'
        ? { ok: true, value: result.value.map((container, index) => ({
            ...container, managedApplicationId: index < managedMembers ? id : null,
          })) }
        : result;
    },
  });
  expect(text('tbody th[scope="row"]')).toContain('▸ immich Not managed Compose project · 2 containers');
  managedMembers = 2;
  await store.load();
  store.toggle('m-pop/project/immich');
  flushSync();
  expect(text('tbody th[scope="row"]')).toContain('▾ immich Managed Compose project · 2 containers');
  expect(text('tr.started th')[0]).toContain(id);
  store.show = 'not-managed';
  flushSync();
  expect(text('tbody th[scope="row"]')).toEqual(['samba Not managed example/samba', 'watchtower Not managed containrrr/watchtower']);
});
