// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, expect, it } from 'vitest';
import { installAnimations } from '$lib/forms/animations.fixture';
import { fakeHosting, type FakeHosting } from '../hosting.fixture';
import MachineList from './MachineList.svelte';
import { MachineStore } from './store.svelte';

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

async function open(): Promise<MachineStore> {
  const store = new MachineStore(calls);
  app = mount(MachineList, { target: document.body, props: { store } });
  await store.load();
  flushSync();
  return store;
}

const rows = () => [...document.querySelectorAll('tbody tr')].map((row) => [...row.children].map((cell) => cell.textContent.trim()).join(' | '));

it('HSE-016: lists each machine that can host by name, with the status words of the Devices page and a link to its device page', async () => {
  await open();
  expect(rows()).toEqual(['mini | Down', 'nas | Up', 'pop-os | Up']);
  expect(document.querySelector('tbody a')?.getAttribute('href')).toMatch(/\/devices\/m-mini$/);
  expect(document.querySelectorAll('thead th')).toHaveLength(2);
});

it('HSE-016: a hub with no machine that can host says how to add one', async () => {
  calls.machines = () => Promise.resolve({ ok: true, value: [] });
  await open();
  expect(document.body.textContent).toContain('No machine can host yet');
  expect(document.querySelector('table')).toBeNull();
});

it('HSE-016: a failed read gives the words of the hub and a retry', async () => {
  calls.machines = () => Promise.resolve({ ok: false, error: { code: 'UNAVAILABLE', message: 'The hub did not answer.' } });
  await open();
  expect(document.body.textContent).toContain('The hub did not answer.');
  expect([...document.querySelectorAll('button')].some((button) => button.textContent.includes('Retry'))).toBe(true);
});
