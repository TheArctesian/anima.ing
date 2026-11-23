// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, expect, it } from 'vitest';
import { installAnimations } from '$lib/forms/animations.fixture';
import { fakeHosting, type FakeHosting } from '../hosting.fixture';
import { VolumeStore } from './store.svelte';
import VolumeList from './VolumeList.svelte';

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

async function open(): Promise<VolumeStore> {
  const store = new VolumeStore(calls);
  app = mount(VolumeList, { target: document.body, props: { store } });
  await store.load();
  flushSync();
  return store;
}

const text = (selector: string) => [...document.querySelectorAll(selector)].map((each) => each.textContent.replace(/\s+/g, ' ').trim());
const cells = () => [...document.querySelectorAll('tbody tr:not(.group)')].map((row) => [...row.children].map((cell) => cell.textContent.replace(/\s+/g, ' ').trim()).join(' | '));

it('HSE-015: reads the volumes of each machine and shows one group for each, with the application that uses a volume and a mark on a volume that nothing uses', async () => {
  await open();
  expect(calls.storageAsked.toSorted()).toEqual(['m-mini', 'm-nas', 'm-pop']);
  expect(text('tr.group th')).toEqual(['mini · No volumes Down', 'nas Up 1 volume, 1 not used', 'pop-os Up 3 volumes, 1 not used. 39.6GB in use, 38.4GB (96%) can be freed by a clean-up']);
  expect(cells()).toEqual([
    'samba-shares | Not used | 0B',
    'immich_pgdata | immich | 1.2GB',
    'watchtower-cache Driver nfs | watchtower | 12kB',
    'old-photos | Not used | 38.4GB',
  ]);
});

it('HSE-015: a machine that does not answer shows why in its group, and the other machines still show their volumes', async () => {
  calls.failing['m-nas'] = 'The device has no SSH credential.';
  await open();
  expect(text('tr.group th')).toContain('nas Up The device has no SSH credential.');
  expect(text('tbody tr:not(.group) th')).toEqual(['immich_pgdata', 'watchtower-cache Driver nfs', 'old-photos']);
});

it('HSE-015: the field finds a volume by its name or by what uses it, on all machines', async () => {
  const store = await open();
  store.search = 'immich';
  flushSync();
  expect(text('tr.group th .name')).toEqual(['pop-os']);
  expect(text('tbody tr:not(.group) th')).toEqual(['immich_pgdata']);
  store.search = 'shares';
  flushSync();
  expect(text('tbody tr:not(.group) th')).toEqual(['samba-shares']);
});
