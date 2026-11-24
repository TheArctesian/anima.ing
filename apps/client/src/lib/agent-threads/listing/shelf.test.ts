import { describe, expect, it } from 'vitest';
import { outcomeFrom } from '../live/run';
import { NO_MARKS, shelfOf, type Activity, type Marks } from './shelf';

// The same cases as the test of `Listing::shelf`, crates/agent-threads/src/control/shelf/tests.rs.
const START = Date.parse('2027-01-15T08:00:00Z');
const WEEK = 7 * 24;
const at = (hours: number): Date => new Date(START + hours * 60 * 60 * 1000);
const quiet = (since: number): Activity => ({ prompted: at(since), last: at(since), working: false, asked: null });
const plain = { pinned: false, archived: false, marks: NO_MARKS };
const marked = (marks: Partial<Marks>, pinned = false) => ({ pinned, archived: false, marks: { ...NO_MARKS, ...marks } });

describe('TH61: where the list shows a thread', () => {
  it('keeps a settled thread settled until the person prompts it', () => {
    const settled = marked({ settledAt: at(2) });
    expect(shelfOf(settled, quiet(1), at(3))).toEqual({ shelf: 'settled', since: at(2), auto: false });
    expect(shelfOf(settled, quiet(4), at(5))).toEqual({ shelf: 'active' });
    expect(shelfOf(marked({ unsettledAt: at(3) }), quiet(1), at(4))).toEqual({ shelf: 'active' });
  });

  it('ends a snooze at its time, on a prompt or on a newer request', () => {
    const snoozed = marked({ snoozedAt: at(2), snoozedUntil: at(10) });
    expect(shelfOf(snoozed, quiet(1), at(9))).toEqual({ shelf: 'snoozed', until: at(10) });
    expect(shelfOf(snoozed, quiet(1), at(10))).toEqual({ shelf: 'active' });
    expect(shelfOf(snoozed, quiet(3), at(4))).toEqual({ shelf: 'active' });
    expect(shelfOf(snoozed, { ...quiet(1), asked: at(3) }, at(4))).toEqual({ shelf: 'active' });
    expect(shelfOf(snoozed, { ...quiet(1), asked: at(1) }, at(4))).toEqual({ shelf: 'snoozed', until: at(10) });
  });

  it('settles a quiet thread by itself after a week, unless it is held', () => {
    expect(shelfOf(plain, quiet(0), at(WEEK - 1))).toEqual({ shelf: 'active' });
    expect(shelfOf(plain, quiet(0), at(WEEK))).toEqual({ shelf: 'settled', since: at(0), auto: true });
    expect(shelfOf(marked({}, true), quiet(0), at(WEEK))).toEqual({ shelf: 'active' });
    expect(shelfOf(plain, { ...quiet(0), asked: at(0) }, at(WEEK))).toEqual({ shelf: 'active' });
    expect(shelfOf(marked({ unsettledAt: at(5) }), quiet(0), at(WEEK))).toEqual({ shelf: 'active' });
    expect(shelfOf(marked({ snoozedAt: at(1), snoozedUntil: at(24) }), quiet(0), at(WEEK + 1))).toEqual({ shelf: 'active' });
  });

  it('reads the marks of a listing, and whether the runner started its thread, as the runner writes them', () => {
    const read = outcomeFrom({
      outcome: 'listings',
      listings: [{ thread: 't', pinned: false, archived: false, removed: false, snoozed_at: '2027-01-15T10:00:00Z', snoozed_until: '2027-01-15T18:00:00Z', started_by_anima: true }],
    });
    expect(read).toEqual({
      outcome: 'listings',
      listings: [
        { thread: 't', pinned: false, archived: false, title: null, removed: false, marks: { ...NO_MARKS, snoozedAt: at(2), snoozedUntil: at(10) }, startedByAnima: true },
      ],
    });
  });
});
