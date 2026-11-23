import { describe, expect, it } from 'vitest';
import type { ListedRow } from '$lib/agent-threads/listing/listing';
import type { Tail } from '$lib/agent-threads/thread';
import type { MenuEntry } from '../kit/menu';
import { intentOf, manyMenu, rowMenu, snoozeItems, type Able } from './actions';
import { customWake, snoozePresets } from './engine';
import { NO_MARKS, type Marks } from '$lib/agent-threads/listing/shelf';
import { arranged, emptyWords, inOrder, inProjects, projectsOf, type Arranging } from './inbox';
import { clicked, kept, NO_SELECTION } from './selection';

const NOW = new Date('2026-11-21T15:00:00Z');
const MINUTE = 60_000;

function row(id: string, tail: Tail, minutesAgo: number, more: Partial<ListedRow> = {}): ListedRow {
  return { id, program: 'claude_code', title: id, folder: '/w/anima', branch: null, model: null, started: null, last: new Date(NOW.getTime() - minutesAgo * MINUTE), tail, pinned: false, archived: false, ...more };
}

const AT: Arranging = { now: NOW, seen: new Map(), marks: new Map() };
const DAY = 24 * 60;
const ids = (items: readonly { row: ListedRow }[]): string[] => items.map((item) => item.row.id);

describe('the parts of the inbox (02 section 5.4, OD-THR55)', () => {
  it('puts pinned cards first, then the active cards with the threads that need the person first', () => {
    const parts = arranged([row('quiet', 'turn_ended', 5), row('asks', 'asks', 30), row('pin', 'turn_ended', 300, { pinned: true }), row('ended', 'turn_ended', 10)], {
      ...AT,
      seen: new Map([['quiet', NOW]]),
    });
    expect(ids(parts.pinned)).toEqual(['pin']);
    expect(ids(parts.active)).toEqual(['ended', 'asks', 'quiet']);
    expect(parts.active.map((item) => item.status)).toEqual(['done', 'approval', null]);
    expect(parts.active.map((item) => item.recedes)).toEqual([false, false, true]);
  });

  it('puts running threads on the Working shelf, and a pinned one stays pinned', () => {
    const parts = arranged([row('run', 'running', 0), row('start', 'starting', 0), row('pin', 'running', 0, { pinned: true })], AT);
    expect(ids(parts.working).sort()).toEqual(['run', 'start']);
    expect(ids(parts.pinned)).toEqual(['pin']);
  });

  it("settles a quiet thread by the runner's rule after 7 days (TH61); a pin or a request keeps one out", () => {
    const parts = arranged([row('old', 'turn_ended', 8 * DAY), row('young', 'turn_ended', 6 * DAY), row('oldpin', 'turn_ended', 8 * DAY, { pinned: true }), row('oldasks', 'asks', 8 * DAY)], AT);
    expect(ids(parts.settled)).toEqual(['old']);
    expect(parts.settled[0]?.since).toEqual(new Date(NOW.getTime() - 8 * DAY * MINUTE));
    expect(ids(parts.pinned)).toEqual(['oldpin']);
    expect(ids(parts.active).sort()).toEqual(['oldasks', 'young']);
  });

  it('reads the marks of the runner: snoozed until a time, settled by hand (also when pinned), and woke until opened', () => {
    const ago = (minutes: number): Date => new Date(NOW.getTime() - minutes * MINUTE);
    const marked = (more: Partial<Marks>): Marks => ({ ...NO_MARKS, ...more });
    const started = { started: ago(60) };
    const marks = new Map([
      ['nap', marked({ snoozedAt: ago(1), snoozedUntil: new Date(NOW.getTime() + 60 * MINUTE) })],
      ['done', marked({ settledAt: ago(1) })],
      ['pin', marked({ settledAt: ago(1) })],
      ['woke', marked({ wokeAt: ago(1) })],
      ['ranout', marked({ snoozedAt: ago(30), snoozedUntil: ago(2) })],
      ['seen', marked({ wokeAt: ago(3) })],
    ]);
    const rows = ['nap', 'done', 'woke', 'ranout', 'seen'].map((id) => row(id, 'turn_ended', 5, started));
    const parts = arranged([...rows, row('pin', 'turn_ended', 5, { ...started, pinned: true })], { ...AT, marks, seen: new Map([['seen', ago(2)]]) });
    expect(ids(parts.snoozed)).toEqual(['nap']);
    expect(parts.snoozed[0]?.wakes).toEqual(new Date(NOW.getTime() + 60 * MINUTE));
    expect(ids(parts.settled).sort()).toEqual(['done', 'pin']);
    const status = (id: string) => parts.active.find((item) => item.row.id === id)?.status;
    expect([status('woke'), status('ranout'), status('seen')]).toEqual(['woke', 'woke', null]);
  });

  it('takes a settled thread back when the person prompts it after the settle', () => {
    const marks = new Map([['back', { ...NO_MARKS, settledAt: new Date(NOW.getTime() - 10 * MINUTE) }]]);
    const back = row('back', 'turn_ended', 5, { started: new Date(NOW.getTime() - 60 * MINUTE) });
    expect(ids(arranged([back], { ...AT, marks }).settled)).toEqual(['back']);
    expect(ids(arranged([back], { ...AT, marks, prompted: new Map([['back', new Date(NOW.getTime() - 5 * MINUTE)]]) }).active)).toEqual(['back']);
  });

  it("settles a session Anima did not start from its last change, as T3 settles what it imports, once the runner says which it started", () => {
    const rows = [row('ours', 'turn_ended', 5), row('theirs', 'turn_ended', 5), row('runs', 'running', 0), row('pin', 'turn_ended', 5, { pinned: true }), row('back', 'turn_ended', 5), row('wrote', 'turn_ended', 5)];
    const marks = new Map([['back', { ...NO_MARKS, unsettledAt: new Date(NOW.getTime() - MINUTE) }]]);
    const at: Arranging = { ...AT, marks, started: new Set(['ours']), prompted: new Map([['wrote', new Date(NOW.getTime() - 5 * MINUTE)]]) };
    const parts = arranged(rows, at);
    expect(ids(parts.settled)).toEqual(['theirs']);
    expect(parts.settled[0]?.since).toEqual(new Date(NOW.getTime() - 5 * MINUTE));
    expect(ids(parts.active).sort()).toEqual(['back', 'ours', 'wrote']);
    expect([ids(parts.working), ids(parts.pinned)]).toEqual([['runs'], ['pin']]);
    expect(arranged(rows, { ...at, started: null }).settled).toEqual([]);
  });

  it('gives the order of the keys: the shelves count only when open', () => {
    const parts = arranged([row('a', 'turn_ended', 5), row('w', 'running', 0), row('old', 'turn_ended', 8 * DAY)], AT);
    expect(ids(inOrder(parts, new Set()))).toEqual(['a']);
    expect(ids(inOrder(parts, new Set(['working', 'settled'])))).toEqual(['a', 'w', 'old']);
  });
});

describe('the project filter (02 section 5.3) and the empty inbox (section 5.10)', () => {
  const rows = [row('a', 'turn_ended', 1, { folder: '/w/zeta' }), row('b', 'turn_ended', 1, { folder: '/w/anima' }), row('c', 'turn_ended', 1, { folder: '/x/anima' }), row('d', 'turn_ended', 1, { folder: null })];

  it('lists each folder once by its name, and keeps two folders of one name apart', () => {
    expect(projectsOf(rows).map((project) => [project.name, project.folder])).toEqual([
      ['anima', '/w/anima'],
      ['anima', '/x/anima'],
      ['No folder', null],
      ['zeta', '/w/zeta'],
    ]);
    expect(ids(inProjects(rows, new Set(['/w/zeta', null])).map((each) => ({ row: each })))).toEqual(['a', 'd']);
    expect(inProjects(rows, new Set())).toBe(rows);
  });

  it('says why the inbox is empty', () => {
    expect(emptyWords([], false)).toBe('No threads yet');
    expect(emptyWords(['anima'], true)).toBe('No active threads in anima');
    expect(emptyWords(['anima', 'zeta'], false)).toBe('No threads in these projects yet');
  });
});

describe('selection (02 section 5.11)', () => {
  const order = ['a', 'b', 'c', 'd'];

  it('a click opens the thread and clears the selection', () => {
    expect(clicked({ ids: new Set(['a']), anchor: 'a' }, 'c', order, { toggle: false, range: false })).toEqual({ selection: { ids: new Set(), anchor: 'c' }, open: true });
  });

  it('⌘-click toggles one row; Shift-click selects the range from the last row clicked', () => {
    const one = clicked(NO_SELECTION, 'b', order, { toggle: true, range: false }).selection;
    expect([...one.ids]).toEqual(['b']);
    expect([...clicked(one, 'b', order, { toggle: true, range: false }).selection.ids]).toEqual([]);
    expect([...clicked(one, 'd', order, { toggle: false, range: true }).selection.ids]).toEqual(['b', 'c', 'd']);
    expect([...clicked(one, 'a', order, { toggle: false, range: true }).selection.ids]).toEqual(['a', 'b']);
  });

  it('drops the rows that left the list', () => {
    const selection = { ids: new Set(['a', 'x']), anchor: 'x' };
    expect(kept(selection, order)).toEqual({ ids: new Set(['a']), anchor: null });
  });
});

describe('the right-click menus (08 inbox 10 and 26)', () => {
  const able: Able = { listing: true, marks: false, why: 'Needs TH61.', settled: false, snoozed: false };
  const [item] = arranged([row('a', 'turn_ended', 5, { branch: 'main', folder: '/w/anima' })], AT).active;
  const snooze = snoozeItems(NOW, (at) => at.toISOString().slice(11, 16), () => 'Mon');
  const labels = (entries: readonly MenuEntry[]): string[] =>
    entries.map((entry) => {
      if (entry.kind === 'line') return '—';
      if (entry.kind === 'group') return `[${entry.label}]`;
      return `${entry.label}${entry.items === undefined ? '' : ' ▸'}${entry.disabled === true ? ' (off)' : ''}`;
    });

  it("keeps T3's order: Snooze and Copy open submenus, and settle and snooze say why they are off", () => {
    if (item === undefined) throw new Error('no row');
    expect(labels(rowMenu(item, able, snooze))).toEqual([
      'Pin thread',
      'Settle thread (off)',
      'Snooze ▸ (off)',
      '[Needs TH61.]',
      '—',
      'Rename thread',
      'Filter by anima',
      '—',
      'Copy ▸',
      '—',
      'Archive thread',
      'Remove from the list',
    ]);
    const copy = rowMenu(item, able, snooze).find((entry) => entry.kind === 'choice' && entry.label === 'Copy');
    expect(copy?.kind === 'choice' ? labels(copy.items ?? []) : []).toEqual(['Path', 'Branch', 'Thread ID']);
    expect(labels(rowMenu(item, { ...able, marks: true, settled: true, snoozed: true, filtered: true }, snooze)).slice(0, 7)).toEqual(['Pin thread', 'Un-settle thread', 'Wake thread', '—', 'Rename thread', 'Show all projects', '—']);
  });

  it('offers "End the program" while the program runs, and then Archive is off', () => {
    if (item === undefined) throw new Error('no row');
    expect(labels(rowMenu(item, { ...able, endable: true }, snooze)).slice(-3)).toEqual(['End the program', 'Archive thread (off)', 'Remove from the list']);
    expect(labels(rowMenu(item, able, snooze))).not.toContain('End the program');
    expect(intentOf('end', [item], NOW)).toEqual({ kind: 'end' });
  });

  it("counts the rows of a multi-selection in T3's order, with no line and no Archive", () => {
    if (item === undefined) throw new Error('no row');
    expect(labels(manyMenu([item, { ...item, row: { ...item.row, pinned: true } }], { ...able, listing: false }, snooze))).toEqual([
      'Unpin (1) (off)',
      'Settle (2) (off)',
      'Snooze (2) ▸ (off)',
      '[Needs TH61.]',
      'Remove from the list (2) (off)',
    ]);
    expect(labels(manyMenu([item, { ...item, state: 'asks' }], { ...able, marks: true }, snooze))).not.toContain('Snooze (2) ▸');
  });

  it('turns each choice into what it does', () => {
    if (item === undefined) throw new Error('no row');
    expect(intentOf('copy-branch', [item], NOW)).toEqual({ kind: 'copy', text: 'main' });
    expect(intentOf('unpin-many', [item], NOW)).toEqual({ kind: 'listing', change: { change: 'pin', on: false }, pinnedOnly: true });
    expect(intentOf('snooze-1h', [item], NOW)).toEqual({ kind: 'mark', change: { change: 'snooze', until: new Date(NOW.getTime() + 60 * MINUTE) } });
    expect(intentOf('snooze', [item], NOW)).toEqual({ kind: 'snooze-menu' });
    expect(intentOf('snooze-custom', [item], NOW)).toEqual({ kind: 'snooze-custom' });
    expect(intentOf('filter-project', [item], NOW)).toEqual({ kind: 'filter', folder: '/w/anima' });
    expect(intentOf('all-projects', [item], NOW)).toEqual({ kind: 'filter', folder: null });
    expect(intentOf('nothing', [item], NOW)).toBeNull();
    expect(intentOf('pin', [], NOW)).toBeNull();
  });
});

describe("T3's snooze presets (08 inbox 6)", () => {
  const ids = (now: Date): string[] => snoozePresets(now).map((preset) => `${preset.label} ${preset.until.getHours().toString()}:${preset.until.getMinutes().toString().padStart(2, '0')}`);

  it('lists this evening until 18:00, then tomorrow and next Monday at 09:00, soonest first', () => {
    // A Wednesday at 10:00.
    expect(ids(new Date(2026, 10, 18, 10, 0))).toEqual(['In 1 hour 11:00', 'In 3 hours 13:00', 'This evening 18:00', 'Tomorrow 9:00', 'Next week 9:00']);
    // At 19:00 the evening has passed.
    expect(ids(new Date(2026, 10, 18, 19, 0))).toEqual(['In 1 hour 20:00', 'In 3 hours 22:00', 'Tomorrow 9:00', 'Next week 9:00']);
  });

  it('keeps one row for one wake minute: on a Sunday, Monday 09:00 is "Tomorrow"', () => {
    expect(ids(new Date(2026, 10, 22, 7, 0)).filter((label) => label.startsWith('Tomorrow'))).toEqual(['Tomorrow 9:00']);
  });

  it('names the day in the detail only when the label does not and the wake is on another day', () => {
    const items = snoozeItems(new Date(2026, 10, 18, 22, 0), (at) => `${at.getHours().toString()}h`, () => 'Thu');
    const details = items.flatMap((entry) => (entry.kind === 'choice' ? [`${entry.label}: ${entry.detail ?? ''}`] : []));
    expect(details).toEqual(['In 1 hour: 23h', 'In 3 hours: Thu 1h', 'Tomorrow: 9h', 'Next week: Thu 9h', 'Custom…: ']);
  });
});

describe('a custom snooze (08 inbox 6)', () => {
  const now = new Date(2026, 10, 18, 10, 0);

  it('wakes at a date and time in the future, or after a positive duration', () => {
    expect(customWake({ mode: 'date', date: '2026-11-19', time: '08:30' }, now)).toEqual(new Date(2026, 10, 19, 8, 30));
    expect(customWake({ mode: 'duration', amount: '2', unit: 'hours' }, now)).toEqual(new Date(now.getTime() + 2 * 3_600_000));
  });

  it('refuses a time that has passed, a bad date and a duration that is not positive', () => {
    expect(customWake({ mode: 'date', date: '2026-11-18', time: '09:00' }, now)).toBeNull();
    expect(customWake({ mode: 'date', date: 'tomorrow', time: '09:00' }, now)).toBeNull();
    expect(customWake({ mode: 'duration', amount: '0', unit: 'days' }, now)).toBeNull();
  });
});
