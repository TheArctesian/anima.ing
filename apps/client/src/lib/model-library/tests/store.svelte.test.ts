import { expect, it } from 'vitest';
import { LibraryStore } from '../store.svelte';
import { fakeLibrary } from './library.fixture';

const make = (calls = fakeLibrary()) => ({ calls, store: new LibraryStore(calls) });

it('FIT-005: the first model is selected with its variant that fits best, and each machine has its row', async () => {
  const { store } = make();
  await store.load();
  expect([store.status, store.selected, store.variant]).toEqual(['ready', 'llama3.3', 'llama3.3:70b-q4_k_m']);
  store.select('qwen3');
  expect(store.variant).toBe('qwen3:8b-q4_k_m');
  expect(store.machines.map((fit) => fit.place.name)).toEqual(['nas', 'pop-os']);
  expect(store.target?.place.name).toBe('pop-os');
});

it('FIT-005: Fits a machine leaves out a model with no variant that runs anywhere', async () => {
  const { store } = make();
  await store.load();
  store.onlyFitting = true;
  expect(store.shown.map((model) => model.name)).toEqual(['qwen3']);
});

it('FIT-004: a variant that fits no machine has no Pull, and a new context reads the fits again', async () => {
  const { calls, store } = make();
  await store.load();
  expect(store.target).toBeNull();
  await store.setContext(32768);
  expect(calls.contexts).toEqual([null, null, 32768, 32768]);
  expect(store.selected).toBe('llama3.3');
});

it('PULL-001: Pull names the machine with the best fit; a refusal keeps the words of the hub', async () => {
  const { calls, store } = make();
  await store.load();
  store.select('qwen3');
  const target = store.target;
  if (target === null) throw new Error('qwen3 fits pop-os');
  await store.pull(target);
  expect(calls.pulls).toEqual([['m-pop-os', 'qwen3:8b-q4_k_m']]);
  expect(store.pulled).toBe('qwen3:8b-q4_k_m to pop-os');
  calls.pullFails = { code: 'VALIDATION', message: 'The input is not valid.', fields: [{ field: 'deviceId', message: 'the machine never asked this hub for work' }] };
  await store.pull(target);
  expect([store.refusal, store.pulled]).toEqual(['The machine never asked this hub for work.', '']);
});
