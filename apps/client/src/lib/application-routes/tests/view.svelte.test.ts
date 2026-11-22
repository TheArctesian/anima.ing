// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { installAnimations } from '$lib/forms/animations.fixture';
import RoutesSection from '../RoutesSection.svelte';
import { RoutesStore } from '../store.svelte';
import { AGENT_REFUSED, NO_SECOND_FACTOR, NOT_LINUX_ERROR, NOW, fakeCalls, route, type FakeCalls } from './routes.fixture';

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

const text = () => document.body.textContent;
const button = (label: string) => [...document.querySelectorAll<HTMLButtonElement>('button')].find((each) => (each.getAttribute('aria-label') ?? each.textContent.trim()) === label);
const row = (name: string) => [...document.querySelectorAll<HTMLButtonElement>('ul[aria-label="Routes"] button')].find((each) => each.textContent.includes(name));
const reasonOf = (control: Element | undefined) => document.getElementById(control?.getAttribute('aria-describedby') ?? '')?.textContent ?? '';

async function open(hosted = true): Promise<RoutesStore> {
  const store = new RoutesStore(calls, hosted, () => NOW);
  app = mount(RoutesSection, { target: document.body, props: { store, now: () => NOW } });
  flushSync();
  await settle();
  return store;
}

it('H21: the list has a row for each route with its kind, access, target, proof, certificate and traffic', async () => {
  await open();
  expect(calls.listAsked).toBe(1);
  expect(text()).toContain('0 serving, 5 waiting');
  expect(row('vault.example.com')?.textContent).toContain('Web · Private');
  expect(row('vault.example.com')?.textContent).toContain('On atlas');
  expect(row('tracks.example.com')?.textContent).toContain('Certificate ends in 9 days');
  expect(row('wiki.example.com')?.textContent).toContain('Waits for TXT record');
  expect(row('media.example.org')?.textContent).toContain('Name lost');
  expect(row('media.example.org')?.textContent).toContain('12 refused');
  expect(row('code.example.com')?.textContent).toContain('Redirect to');
  expect(text()).toContain('Streams of TCP and UDP need a hub that you host');
});

it('H21: a hub that a person hosts has no note about streams', async () => {
  await open(false);
  expect(text()).not.toContain('Streams of TCP and UDP need a hub that you host');
});

it('H21: Needs you keeps only the routes that need the operator', async () => {
  await open();
  [...document.querySelectorAll<HTMLButtonElement>('[role="radio"]')].find((each) => each.textContent.startsWith('Needs you'))?.click();
  flushSync();
  expect(row('vault.example.com')).toBeUndefined();
  expect(row('wiki.example.com')).toBeDefined();
});

it('H21: Add a route is off with the reason that the hub keeps no record of the applications; a click asks nothing', async () => {
  await open();
  const add = button('Add a route');
  expect(add?.getAttribute('aria-disabled')).toBe('true');
  expect(reasonOf(add)).toContain('task H6');
  add?.click();
  await settle();
  expect(calls.listAsked).toBe(1);
});

it('H21: no routes shows the empty state, which says that a new route is private', async () => {
  calls.routes = [];
  await open();
  expect(text()).toContain('No routes yet');
  expect(text()).toContain('A new route is private');
  expect(button('Add a route')?.getAttribute('aria-disabled')).toBe('true');
});

it('H21: the loading state shows while the hub answers, and a failed list shows its reason and a retry', async () => {
  calls.listFails = { code: 'unreachable', message: 'The hub did not answer.' };
  const store = new RoutesStore(calls, true, () => NOW);
  app = mount(RoutesSection, { target: document.body, props: { store } });
  flushSync();
  expect(document.querySelector('[aria-label="Loading the routes"]')).not.toBeNull();
  await settle();
  expect(text()).toContain('The hub did not answer.');
  calls.listFails = undefined;
  button('Retry')?.click();
  await settle();
  expect(calls.listAsked).toBe(2);
  expect(row('vault.example.com')).toBeDefined();
});

it('R-366: a hub that is not on Linux shows its sentence in place of the list after Check all', async () => {
  await open();
  calls.checkFails = NOT_LINUX_ERROR;
  button('Check all')?.click();
  await settle();
  expect(text()).toContain('Routing needs a hub on Linux.');
  expect(document.querySelector('ul[aria-label="Routes"]')).toBeNull();
});

it('H21: the page of a route gives its state, its checks, its traffic and Make public off with the reason of OD-HOST-43', async () => {
  await open();
  row('vault.example.com')?.click();
  flushSync();
  expect(document.querySelector('h3')?.textContent).toBe('vault.example.com');
  expect(text()).toContain('Waits for consent at the machine');
  const checks = document.querySelector('ul[aria-label="Checks"]')?.textContent ?? '';
  expect(checks).toContain('The name is proven');
  expect(checks).toContain('Not given');
  expect(checks).toContain('Valid');
  const makePublic = document.querySelector('section[aria-label="Make public"] button');
  expect(makePublic?.getAttribute('aria-disabled')).toBe('true');
  expect(reasonOf(makePublic ?? undefined)).toContain('Consent at atlas is not given');
  expect(document.querySelector('section[aria-label="Traffic"]')?.textContent).toContain('9 MB');
  expect(document.querySelector('section[aria-label="DNS records"]')).toBeNull();
  button('Routes')?.click();
  flushSync();
  expect(row('vault.example.com')).toBeDefined();
});

it('H21: a public route has no Make public', async () => {
  await open();
  row('media.example.org')?.click();
  flushSync();
  expect(document.querySelector('section[aria-label="Make public"]')).toBeNull();
});

it('H21: a name that is not proven shows the TXT record with Copy, and says that the address of the door is not given', async () => {
  await open();
  row('wiki.example.com')?.click();
  flushSync();
  const records = document.querySelector('section[aria-label="DNS records"]')?.textContent ?? '';
  expect(records).toContain('_anima.wiki.example.com');
  expect(records).toContain('anima-route=hub_1.wiki');
  expect(records).toContain('The hub does not give it to this screen yet');
  expect([...document.querySelectorAll('section[aria-label="DNS records"] button')].filter((each) => each.textContent.trim() === 'Copy')).toHaveLength(3);
});

it('H21: on a hub that a person hosts the record is the name at the address of the hub, with no token', async () => {
  calls.routes = [route({ proof: 'WAITING', waitsFor: 'PROOF', proofRecordName: null, proofRecordValue: null })];
  await open(false);
  row('vault.example.com')?.click();
  flushSync();
  const record = document.querySelector('section[aria-label="DNS record"]')?.textContent ?? '';
  expect(record).toContain('Point the name at this hub');
  expect(record).toContain('the public address of this hub');
});

it('SF-232: Check now shows the refusal of the hub on the page of the route', async () => {
  await open();
  row('vault.example.com')?.click();
  flushSync();
  calls.checkFails = NO_SECOND_FACTOR;
  button('Check now')?.click();
  await settle();
  expect(calls.checked).toEqual(['route-vault']);
  expect(document.querySelector('[role="alert"]')?.textContent).toContain('Turn on the second factor');
});

it('H21: Remove asks first, then removes the route and shows the list; a refusal stays in the dialog', async () => {
  await open();
  row('vault.example.com')?.click();
  flushSync();
  button('Remove route')?.click();
  flushSync();
  const dialog = () => document.querySelector('[role="dialog"]');
  expect(dialog()?.textContent).toContain('You cannot undo this.');
  expect(dialog()?.textContent).toContain('vault.example.com stops reaching');
  expect(dialog()?.textContent).toContain('Delete the DNS records');
  calls.removeFails = AGENT_REFUSED;
  [...(dialog()?.querySelectorAll<HTMLButtonElement>('button') ?? [])].find((each) => each.textContent.trim() === 'Remove route')?.click();
  await settle();
  expect(dialog()?.textContent).toContain('never by an AI agent');
  calls.removeFails = undefined;
  [...(dialog()?.querySelectorAll<HTMLButtonElement>('button') ?? [])].find((each) => each.textContent.trim() === 'Remove route')?.click();
  await settle();
  expect(calls.removed).toEqual(['route-vault', 'route-vault']);
  expect(row('vault.example.com')).toBeUndefined();
  expect(row('wiki.example.com')).toBeDefined();
});
