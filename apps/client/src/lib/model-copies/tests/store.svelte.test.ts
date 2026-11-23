import { expect, it } from 'vitest';
import { CopiesStore } from '../store.svelte';
import { fakeCopies } from './copies.fixture';

const make = (calls = fakeCopies()) => ({ calls, store: new CopiesStore(calls) });

it('HSE-013: the list keeps what the hub answers, and a refusal keeps its words', async () => {
  const { calls, store } = make();
  await store.load();
  expect([store.status, store.copies.length, store.moving]).toEqual(['ready', 3, true]);
  calls.list = () => Promise.resolve({ ok: false, error: { code: 'unreachable', message: 'The hub did not answer.' } });
  await store.load();
  expect([store.status, store.problem]).toEqual(['failed', 'The hub did not answer.']);
});

it('PULL-001: a pull names the machine and the variant, and its copy joins the list', async () => {
  const { calls, store } = make();
  await store.load();
  const result = await store.pull('m-nas', 'gemma3:4b-q8_0');
  expect(result.ok).toBe(true);
  expect(calls.pulls).toEqual([['m-nas', 'gemma3:4b-q8_0']]);
  expect(store.copies.find((copy) => copy.id === 'c-m-nas-new')?.state).toBe('QUEUED');
});

it('PULL-001: a pull that the hub refuses changes nothing and gives the words of the hub', async () => {
  const { calls, store } = make();
  await store.load();
  calls.pullFails = { code: 'VALIDATION', message: 'That machine already has this variant.', fields: [] };
  const result = await store.pull('m-pop', 'qwen3:8b-q4_k_m');
  expect(result.ok ? '' : result.error.message).toBe('That machine already has this variant.');
  expect(store.copies.length).toBe(3);
});

it('PULL-009: a removal keeps the row as Removing, and a copy that went at once leaves the list', async () => {
  const { calls, store } = make();
  await store.load();
  await store.remove('c-pop-gemma');
  expect(store.copies.find((copy) => copy.id === 'c-pop-gemma')?.state).toBe('REMOVING');
  calls.removeAnswer = { ok: true, value: null };
  await store.remove('c-nas-qwen');
  expect(store.copies.some((copy) => copy.id === 'c-nas-qwen')).toBe(false);
  expect(calls.removals).toEqual(['c-pop-gemma', 'c-nas-qwen']);
});

it('HSE-013: the choice of a machine that has no copy any more goes back to all machines', async () => {
  const { calls, store } = make();
  await store.load();
  store.machine = 'm-nas';
  calls.copies = calls.copies.filter((copy) => copy.place.id !== 'm-nas');
  await store.load(true);
  expect(store.machine).toBe('');
});
