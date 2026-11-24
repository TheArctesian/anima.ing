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

it('PULL-012: the list keeps the found models, and Use adds the copy and takes the model off the found list', async () => {
  const { calls, store } = make();
  await store.load();
  expect(store.found.map((model) => model.name)).toEqual(['qwen3:8b', 'Qwen/Qwen3-8B/model.safetensors']);
  const model = store.found[0];
  if (model === undefined) throw new Error('no found model');
  const result = await store.use(model);
  expect(result.ok).toBe(true);
  expect(calls.uses).toEqual([['m-pop', 'a'.repeat(64)]]);
  expect(store.copies.find((copy) => copy.origin === 'FOUND')?.address).toBe('qwen3:8b');
  expect(store.found.map((each) => each.found)).toEqual(['b'.repeat(64)]);
});

it('PULL-012: Find models asks each machine that has asked for work, and the page then reads again', async () => {
  const { calls, store } = make();
  const result = await store.findAll();
  expect(result).toEqual({ ok: true, value: 2 });
  expect(calls.finds).toEqual(['m-nas', 'm-pop']);
  expect(store.finding).toBe(true);
});

it('PULL-001: a refusal with a reason for a field gives that reason, not the general sentence', async () => {
  const { calls, store } = make();
  await store.load();
  calls.pullFails = {
    code: 'VALIDATION',
    message: 'Some input is not valid.',
    fields: [{ field: 'deviceId', message: 'the machine never asked this hub for work' }],
  };
  const result = await store.pull('m-nas', 'gemma3:4b-q8_0');
  expect(result.ok ? '' : result.error.message).toBe('The machine never asked this hub for work.');
});

it('PULL-012: the refusal of Use stays on the page until the next action', async () => {
  const { store } = make();
  await store.load();
  const model = store.found[0];
  if (model === undefined) throw new Error('no found model');
  await store.use({ ...model, found: 'c'.repeat(64) });
  expect(store.refusal).toBe('The last model report of that machine lists no such model.');
  await store.remove('c-pop-gemma');
  expect(store.refusal).toBe('');
});

it('FIT-023: Pin keeps the idle seconds of the load, and the row says it is asked until the machine reports the pin', async () => {
  const { calls, store } = make();
  await store.load();
  await store.keep('c-pop-gemma', true, 900);
  expect(calls.keeps).toEqual([['c-pop-gemma', true, 900]]);
  expect(store.asked).toEqual({ 'c-pop-gemma': true });
  await store.load();
  expect(store.asked).toEqual({ 'c-pop-gemma': true });
  calls.copies = calls.copies.map((each) => (each.id === 'c-pop-gemma' ? { ...each, load: { state: 'READY', idleSeconds: 900, pinned: true } } : each));
  await store.load();
  expect(store.asked).toEqual({});
});

it('FIT-023: a Pin that the hub refuses changes no row and gives the words of the hub', async () => {
  const { calls, store } = make();
  await store.load();
  calls.keepFails = { code: 'VALIDATION', message: 'The machine does not have this copy now.', fields: [] };
  await store.keep('c-pop-gemma', true, 900);
  expect([store.asked, store.keepRefusal]).toEqual([{}, 'The machine does not have this copy now.']);
});
