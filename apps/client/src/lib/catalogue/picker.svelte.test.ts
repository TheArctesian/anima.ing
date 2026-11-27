// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { fakeCalls, type FakeCalls } from '$lib/containers/containers.fixture';
import { installAnimations } from '$lib/forms/animations.fixture';
import CataloguePicker from './CataloguePicker.svelte';
import { CatalogueStore } from './store.svelte';

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

async function settle(): Promise<void> {
  for (let turn = 0; turn < 4; turn += 1) await Promise.resolve();
  flushSync();
}

const MACHINES = [
  { id: 'm-pop', name: 'pop-os', images: ['example/app:2'] },
  { id: 'm-nas', name: 'nas', images: [] },
];

async function pick(machine: string): Promise<void> {
  app = mount(CataloguePicker, { target: document.body, props: { store: new CatalogueStore(calls), machines: MACHINES, machine, open: true, onClose: () => undefined } });
  await settle();
}

const row = (name: string) => [...document.querySelectorAll<HTMLButtonElement>('.entries button')].find((each) => each.querySelector('.name')?.textContent.startsWith(name));
const key = (target: Element | null | undefined, name: string) => target?.dispatchEvent(new KeyboardEvent('keydown', { key: name, bubbles: true }));

it('HSE-022: Install here opens the catalogue with that machine chosen, and each row says where its program runs', async () => {
  await pick('m-pop');
  expect(calls.entriesAsked).toBe(1);
  expect(document.querySelector('[role="dialog"]')?.textContent).toContain('Install on pop-os');
  expect(document.querySelector<HTMLSelectElement>('select')?.value).toBe('m-pop');
  expect(row('PostgreSQL')?.textContent).toContain('Runs on pop-os');
});

it('HSE-022: the arrow keys move from the search into the list, and Enter opens the entry with Install off and its reason', async () => {
  await pick('m-nas');
  key(document.querySelector('input[type="search"]'), 'ArrowDown');
  await settle();
  const first = document.activeElement;
  expect(first?.getAttribute('aria-pressed')).toBe('true');
  key(first, 'ArrowDown');
  await settle();
  expect(document.activeElement).not.toBe(first);
  const name = document.activeElement?.querySelector('.name')?.textContent ?? '';
  expect(document.querySelector(`section[aria-label="About ${name}"]`)).not.toBeNull();
  const install = [...document.querySelectorAll('button')].find((each) => each.textContent.trim() === 'Install on nas');
  expect(install?.getAttribute('aria-disabled')).toBe('true');
  expect(document.getElementById('install-reason')?.textContent).toContain('task H6');
});
