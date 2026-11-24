import { expect, it } from 'vitest';
import { isMoving, machineChoice, machineGroups, reasonWords } from '../rows';
import { CHOICES, COPIES, copy } from './copies.fixture';

it('HSE-013: one group for each machine by name, the copies by address, each with its place, its state and its size', () => {
  const groups = machineGroups(COPIES, '');
  expect(groups.map((group) => [group.name, group.summary])).toEqual([
    ['nas', '1 model · 5.0 GiB'],
    ['pop-os', '2 models · 9.0 GiB'],
  ]);
  expect(groups[1]?.rows.map((row) => [row.address, row.state, row.what])).toEqual([
    ['gemma3:4b-q8_0', 'Ready', 'GGUF · Q8_0 · 1 file · 4.0 GiB'],
    ['qwen3:8b-q4_k_m', 'Downloading', 'GGUF · Q4_K_M · 1 file · 5.0 GiB'],
  ]);
});

it('HSE-013: the choice of a machine narrows the list to it', () => {
  expect(machineGroups(COPIES, 'm-nas').map((group) => group.name)).toEqual(['nas']);
});

it('PULL-007: a pull that runs shows its share, its bytes, its speed and the time left', () => {
  const row = machineGroups(COPIES, 'm-pop')[0]?.rows.find((each) => each.address.startsWith('qwen3'));
  expect(row?.percent).toBe(40);
  expect(row?.progress).toBe('40 % · 2.0 GiB of 5.0 GiB · 10.0 MiB/s · 5 min left');
  const done = machineGroups([copy({ id: 'x' })], '')[0]?.rows[0];
  expect([done?.percent, done?.progress]).toEqual([null, '']);
});

it('PULL-011: a failure says its reason in words, and a code that the client does not know as it is', () => {
  expect(machineGroups(COPIES, 'm-nas')[0]?.rows[0]?.reason).toBe('A file did not match its SHA-256 value.');
  expect(reasonWords('new_code')).toBe('new_code');
  expect(reasonWords(null)).toBe('');
});

it('PULL-009: a copy that is removed offers no second removal', () => {
  const row = machineGroups([copy({ id: 'x', state: 'REMOVING' })], '')[0]?.rows[0];
  expect([row?.state, row?.removing]).toEqual(['Removing', true]);
});

it('PULL-007: the section reads again only while a copy changes on its own', () => {
  expect(isMoving(COPIES)).toBe(true);
  expect(isMoving([copy({ id: 'x', state: 'VALIDATED' }), copy({ id: 'y', state: 'FAILED' })])).toBe(false);
});

it('CMP-002: a machine that takes no model work now says why beside its name', () => {
  expect(CHOICES.machines.map(machineChoice)).toEqual(['nas (in its quiet hours)', 'new (never asked this hub for work)', 'pop-os']);
});

it('PULL-012: a found copy says where it is in place of a quantisation', () => {
  const found = copy({ id: 'c-found', address: 'qwen3:8b', origin: 'FOUND', quantisation: '', repository: 'lm_studio' });
  expect(machineGroups([found], '')[0]?.rows[0]?.what).toBe('GGUF · where LM Studio keeps it · 1 file · 5.0 GiB');
  expect(reasonWords('copy_changed')).toBe('A file of the found model changed where it is.');
});

it('FIT-022, FIT-023: a copy that a server holds says so, with its pin or the idle time before its unload', () => {
  const ready = machineGroups(COPIES, 'm-pop')[0]?.rows[0];
  expect([ready?.load, ready?.pinned, ready?.idleSeconds, ready?.keepable]).toEqual(['In memory · unloads after 15 min with no use', false, 900, true]);
  const pinned = copy({ id: 'p', load: { state: 'ACTIVE', idleSeconds: 3600, pinned: true } });
  expect(machineGroups([pinned], '')[0]?.rows[0]?.load).toBe('In memory, answering · pinned');
  const loading = machineGroups([copy({ id: 'q', state: 'DOWNLOADING' })], '')[0]?.rows[0];
  expect([loading?.load, loading?.keepable]).toEqual(['', false]);
});

it('FIT-023: a pin that the machine has not reported yet says that it is asked', () => {
  const asked = { 'c-pop-gemma': true, 'c-off': true };
  const rows = machineGroups([...COPIES, copy({ id: 'c-off', address: 'phi4:14b' })], 'm-pop', asked)[0]?.rows;
  expect(rows?.map((row) => [row.address, row.load, row.pinned])).toEqual([
    ['gemma3:4b-q8_0', 'In memory · pin asked', true],
    ['phi4:14b', 'Pin asked', true],
    ['qwen3:8b-q4_k_m', '', false],
  ]);
});
