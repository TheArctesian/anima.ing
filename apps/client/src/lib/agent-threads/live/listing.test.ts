import { describe, expect, it } from 'vitest';
import { orderedThreads } from '../state';
import type { ThreadRow } from '../thread';
import { listed, titleOf, type Listing } from './listing';
import { outcomeFrom } from './run';

function row(id: string, title: string): ThreadRow {
  const at = new Date('2026-11-21T10:00:00Z');
  return { id, program: 'claude_code', title, folder: '/work', branch: null, model: null, started: at, last: at, tail: 'turn_ended' };
}

function listing(thread: string, fields: Partial<Listing>): Listing {
  return { thread, pinned: false, archived: false, title: null, removed: false, ...fields };
}

describe('TH46: how the person lists a thread', () => {
  const rows = [row('a', 'First'), row('b', 'Second'), row('c', 'Third'), row('d', 'Fourth')];
  const listings = new Map([
    ['a', listing('a', { title: 'My title', pinned: true })],
    ['b', listing('b', { archived: true })],
    ['c', listing('c', { removed: true })],
  ]);

  it('shows the title of the person, hides a removed thread, and shows an archived one only with the archived', () => {
    expect(listed(rows, listings, false).map((each) => [each.id, each.title])).toEqual([
      ['a', 'My title'],
      ['d', 'Fourth'],
    ]);
    expect(listed(rows, listings, true).map((each) => each.id)).toEqual(['b']);
  });

  it('puts a pinned thread in the group "Pinned" above "Earlier"', () => {
    const now = new Date('2026-11-21T12:00:00Z');
    const seen = new Map([
      ['a', now],
      ['d', now],
    ]);
    const entries = orderedThreads(listed(rows, listings, false), now, seen);
    expect(entries.map((entry) => (entry.kind === 'group' ? entry.title : entry.key))).toEqual(['Pinned', 'a', 'Earlier', 'd']);
  });

  it('takes a title of one line with no hidden mark, as the runner does', () => {
    expect(titleOf('  Fix the login  ')).toBe('Fix the login');
    expect(titleOf('   ')).toBeNull();
    expect(titleOf('a\nb')).toBeNull();
    expect(titleOf(`pay${String.fromCodePoint(0x202e)}evil`)).toBeNull();
    expect(titleOf('x'.repeat(201))).toBeNull();
  });

  it('reads the listings of the runner and its refusal of a thread that runs', () => {
    expect(outcomeFrom({ outcome: 'listings', listings: [{ thread: 'a', pinned: true }] })).toEqual({
      outcome: 'listings',
      listings: [listing('a', { pinned: true })],
    });
    expect(outcomeFrom({ outcome: 'listings', listings: [{ pinned: true }] })).toBeNull();
    expect(outcomeFrom({ outcome: 'refused', why: { refusal: 'running' } })).toEqual({ outcome: 'refused', why: 'running' });
  });
});
