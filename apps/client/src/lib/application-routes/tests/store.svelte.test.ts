import { expect, it } from 'vitest';
import { RoutesStore } from '../store.svelte';
import { NO_SECOND_FACTOR, NO_SUCH_ROUTE, NOT_LINUX_ERROR, NOW, ROUTES, fakeCalls, route } from './routes.fixture';

const make = (calls = fakeCalls()) => ({ calls, store: new RoutesStore(calls, true, () => NOW) });

it('H21: the list is read once on open, and again on load', async () => {
  const { calls, store } = make();
  await store.open();
  await store.open();
  expect(calls.listAsked).toBe(1);
  expect(store.status).toBe('ready');
  expect(store.visible.map((each) => each.name)[0]).toBe('code.example.com');
  await store.load();
  expect(calls.listAsked).toBe(2);
});

it('H21: a list that the hub does not give keeps its reason', async () => {
  const { calls, store } = make();
  calls.listFails = { code: 'unreachable', message: 'The hub did not answer.' };
  await store.open();
  expect(store.status).toBe('failed');
  expect(store.problem).toBe('The hub did not answer.');
});

it('H21: a check keeps the state that the hub answers', async () => {
  const { calls, store } = make();
  await store.open();
  calls.checkAnswers.set('route-wiki', route({ id: 'route-wiki', name: 'wiki.example.com', proof: 'PROVEN', waitsFor: 'MACHINE_CONSENT' }));
  await store.check('route-wiki');
  expect(store.routes.find((each) => each.id === 'route-wiki')?.proof).toBe('PROVEN');
  expect(store.checking.size).toBe(0);
});

it('SF-232: a check that the hub refuses keeps the reason of the hub for that route only', async () => {
  const { calls, store } = make();
  await store.open();
  calls.checkFails = NO_SECOND_FACTOR;
  await store.check('route-vault');
  expect(store.checkProblems.get('route-vault')).toBe(NO_SECOND_FACTOR.message);
  expect(store.checkProblems.has('route-wiki')).toBe(false);
  calls.checkFails = undefined;
  await store.check('route-vault');
  expect(store.checkProblems.has('route-vault')).toBe(false);
});

it('R-366: the refusal of a hub that is not on Linux stops Check all and marks the hub', async () => {
  const { calls, store } = make();
  await store.open();
  calls.checkFails = NOT_LINUX_ERROR;
  await store.checkAll();
  expect(store.notLinux).toBe(true);
  expect(calls.checked).toEqual(['route-vault']);
});

it('H21: Check all checks each route in turn', async () => {
  const { calls, store } = make();
  await store.open();
  await store.checkAll();
  expect(calls.checked).toEqual(ROUTES.map((each) => each.id));
});

it('H21: a removal takes the route out of the list and shows the list again; a refused one changes nothing', async () => {
  const { calls, store } = make();
  await store.open();
  store.select('route-vault');
  calls.removeFails = NO_SUCH_ROUTE;
  expect((await store.remove('route-vault')).ok).toBe(false);
  expect(store.shown?.id).toBe('route-vault');
  calls.removeFails = undefined;
  expect((await store.remove('route-vault')).ok).toBe(true);
  expect(store.selected).toBeNull();
  expect(store.routes.map((each) => each.id)).not.toContain('route-vault');
});
