import { describe, expect, it } from 'vitest';
import type { ListedRow } from '$lib/agent-threads/listing/listing';
import type { Tail } from '$lib/agent-threads/thread';
import { intentOf, manyMenu, rowMenu, type Able } from './actions';
import type { InboxMark } from './engine';
import { arranged, emptyWords, inOrder, inProjects, projectsOf, type Arranging } from './inbox';
import { clicked, kept, NO_SELECTION } from './selection';

const NOW = new Date('2026-11-21T15:00:00Z');
const MINUTE = 60_000;

function row(id: string, tail: Tail, minutesAgo: number, more: Partial<ListedRow> = {}): ListedRow {
  return { id, program: 'claude_code', title: id, folder: '/w/anima', branch: null, model: null, started: null, last: new Date(NOW.getTime() - minutesAgo * MINUTE), tail, pinned: false, archived: false, ...more };
}

const AT: Arranging = { now: NOW, seen: new Map(), marks: new Map(), engineReady: false };
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

  it('until the runner keeps settle (TH61), the quiet threads older than a day stand in Settled; a pin keeps one out', () => {
    const parts = arranged([row('old', 'turn_ended', 2 * 24 * 60), row('oldpin', 'turn_ended', 2 * 24 * 60, { pinned: true }), row('oldasks', 'asks', 2 * 24 * 60)], AT);
    expect(ids(parts.settled)).toEqual(['old']);
    expect(ids(parts.pinned)).toEqual(['oldpin']);
    expect(ids(parts.active)).toEqual(['oldasks']);
    expect(arranged([row('old', 'turn_ended', 2 * 24 * 60)], { ...AT, engineReady: true }).settled).toEqual([]);
  });

  it('reads the marks of the engine: snoozed until a time, settled, and woke', () => {
    const mark = (settledAt: Date | null, snoozedUntil: Date | null, wokeAt: Date | null = null): InboxMark => ({ settledAt, snoozedUntil, wokeAt });
    const marks = new Map([
      ['nap', mark(null, new Date(NOW.getTime() + 60 * MINUTE))],
      ['done', mark(new Date(NOW.getTime() - MINUTE), null)],
      ['woke', mark(null, new Date(NOW.getTime() - MINUTE), new Date(NOW.getTime() - MINUTE))],
      ['pin', mark(new Date(NOW.getTime() - MINUTE), null)],
    ]);
    const parts = arranged([row('nap', 'turn_ended', 5), row('done', 'turn_ended', 5), row('woke', 'turn_ended', 5), row('pin', 'turn_ended', 5, { pinned: true })], { ...AT, marks, engineReady: true });
    expect(ids(parts.snoozed)).toEqual(['nap']);
    expect(parts.snoozed[0]?.wakes).toEqual(new Date(NOW.getTime() + 60 * MINUTE));
    expect(ids(parts.settled).sort()).toEqual(['done', 'pin']);
    expect(parts.active.find((item) => item.row.id === 'woke')?.status).toBe('woke');
  });

  it('gives the order of the keys: the shelves count only when open', () => {
    const parts = arranged([row('a', 'turn_ended', 5), row('w', 'running', 0), row('old', 'turn_ended', 3 * 24 * 60)], AT);
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

describe('the right-click menus (02 section 5.11)', () => {
  const able: Able = { listing: true, marks: false, why: 'Needs TH61.', settled: false, snoozed: false };
  const [item] = arranged([row('a', 'turn_ended', 5, { branch: 'main' })], AT).active;
  const labels = (entries: ReturnType<typeof rowMenu>): string[] => entries.map((entry) => (entry.kind === 'line' ? '—' : entry.kind === 'group' ? `[${entry.label}]` : `${entry.label}${entry.disabled === true ? ' (off)' : ''}`));

  it("keeps T3's order, and shows settle and snooze disabled with the reason until the runner keeps them", () => {
    if (item === undefined) throw new Error('no row');
    expect(labels(rowMenu(item, able))).toEqual([
      'Pin thread',
      'Settle thread (off)',
      'Snooze… (off)',
      '[Needs TH61.]',
      '—',
      'Rename thread',
      '—',
      '[Copy]',
      'Path',
      'Branch',
      'Thread ID',
      '—',
      'Archive thread',
      'Remove from the list',
    ]);
    expect(labels(rowMenu(item, { ...able, marks: true, settled: true, snoozed: true })).slice(0, 3)).toEqual(['Pin thread', 'Un-settle thread', 'Wake thread']);
  });

  it('offers "End the program" above Archive only while the program runs', () => {
    if (item === undefined) throw new Error('no row');
    expect(labels(rowMenu(item, { ...able, endable: true })).slice(-3)).toEqual(['End the program', 'Archive thread', 'Remove from the list']);
    expect(labels(rowMenu(item, able))).not.toContain('End the program');
    expect(intentOf('end', [item], NOW)).toEqual({ kind: 'end' });
  });

  it('counts the rows of a multi-selection', () => {
    if (item === undefined) throw new Error('no row');
    expect(labels(manyMenu([item, { ...item, row: { ...item.row, pinned: true } }], { ...able, listing: false }))).toEqual([
      'Unpin (1) (off)',
      'Settle (2) (off)',
      'Snooze (2)… (off)',
      '[Needs TH61.]',
      '—',
      'Archive (2) (off)',
      'Remove from the list (2) (off)',
    ]);
  });

  it('turns each choice into what it does', () => {
    if (item === undefined) throw new Error('no row');
    expect(intentOf('copy-branch', [item], NOW)).toEqual({ kind: 'copy', text: 'main' });
    expect(intentOf('unpin-many', [item], NOW)).toEqual({ kind: 'listing', change: { change: 'pin', on: false }, pinnedOnly: true });
    expect(intentOf('snooze-1h', [item], NOW)).toEqual({ kind: 'mark', change: { change: 'snooze', until: new Date(NOW.getTime() + 60 * MINUTE) } });
    expect(intentOf('snooze', [item], NOW)).toEqual({ kind: 'snooze-menu' });
    expect(intentOf('nothing', [item], NOW)).toBeNull();
    expect(intentOf('pin', [], NOW)).toBeNull();
  });
});
