import { describe, expect, it } from 'vitest';
import type { RunRow } from '$lib/agent-threads/live/run';
import { detailedRun, detailsOf, fitsInline, laneRoom } from './place';
import { DetailsShown } from './shown.svelte';

const ROW: RunRow = {
  id: 'th-1',
  program: 'claude_code',
  folder: '/work/anima/wt-a',
  mode: 'ask',
  model: null,
  state: 'idle',
  session: null,
  title: 'A thread',
  started: new Date(0),
  ended: null,
  open: [],
};

describe('detailsOf', () => {
  it('names the worktree, the branch and the context window of a run', () => {
    const details = detailsOf({
      ...ROW,
      workspace: { kind: 'worktree', top: '/work/anima/wt-a', branch: 'code/details-card' },
      usage: { context: 41_000, window: 200_000, input: 10, output: 20, costUsd: null },
    });
    expect(details).toEqual({
      runContext: { environments: [], mode: 'local', path: '/work/anima/wt-a', locked: true, worktreePath: '/work/anima/wt-a' },
      branch: { name: 'code/details-card' },
      context: { used: 41_000, max: 200_000 },
    });
  });

  it('shows a checkout with no worktree path, and leaves out a detached branch and an unknown window', () => {
    const details = detailsOf({
      ...ROW,
      workspace: { kind: 'checkout', top: '/work/anima', branch: null },
      usage: { context: 900, window: null, input: 1, output: 1, costUsd: 0.01 },
    });
    expect(details.runContext?.worktreePath).toBeUndefined();
    expect(details.branch).toBeUndefined();
    expect(details.context).toEqual({ used: 900 });
  });

  it('draws nothing for a folder with no Git, or a run the runner said nothing of', () => {
    expect(detailsOf({ ...ROW, workspace: { kind: 'folder', top: ROW.folder, branch: null }, usage: null })).toEqual({});
    expect(detailsOf(ROW)).toEqual({});
  });
});

describe('detailedRun', () => {
  it('gives the run of the open thread only when the card has something of it to show', () => {
    const known: RunRow = { ...ROW, workspace: { kind: 'checkout', top: ROW.folder, branch: 'dev' } };
    expect(detailedRun(known, 'th-1')).toBe(known);
    expect(detailedRun(known, 'th-2')).toBeUndefined();
    expect(detailedRun(ROW, 'th-1')).toBeUndefined();
    expect(detailedRun(undefined, 'th-1')).toBeUndefined();
  });

  it('has no card for a scratch folder whose only part, the context window, is hidden by default', () => {
    const scratch: RunRow = { ...ROW, workspace: { kind: 'folder', top: ROW.folder, branch: null }, usage: { context: 900, window: 200_000, input: 1, output: 1, costUsd: null } };
    expect(detailedRun(scratch, 'th-1')).toBeUndefined();
    expect(detailedRun(scratch, 'th-1', { context: true })).toBe(scratch);
  });

  it('keeps the card when the person hid its one part, so the menu can show it again', () => {
    const known: RunRow = { ...ROW, workspace: { kind: 'checkout', top: ROW.folder, branch: 'dev' } };
    expect(detailedRun(known, 'th-1', { branch: false, environment: false })).toBe(known);
  });
});

describe('fitsInline', () => {
  it('keeps the card inline only while the lane keeps 640 px beside it', () => {
    expect(fitsInline({ width: 1012, height: 600 })).toBe(true);
    expect(fitsInline({ width: 1011, height: 600 })).toBe(false);
    expect(fitsInline({ width: 1400, height: 180 })).toBe(false);
  });
});

describe('laneRoom', () => {
  /** Where a lane of at most `laneMax` px, centred with 48 px at its sides, ends when `room` px are left at the end. */
  function laneEnd(width: number, laneMax: number, room: number): number {
    const left = width - room;
    return Math.min(left / 2 + laneMax / 2, left - 48);
  }

  it('moves the lane so it ends 32 px before the card', () => {
    const width = 1100;
    const room = laneRoom(width, 736);
    expect(room).toBeGreaterThan(0);
    expect(laneEnd(width, 736, room)).toBeLessThanOrEqual(width - 12 - 280 - 32);
  });

  it('needs no room when a narrow lane already ends before the card', () => {
    expect(laneRoom(2400, 736)).toBe(0);
  });
});

describe('DetailsShown', () => {
  it('shows the inline card of each thread until it is hidden, and opens a popover only on demand', () => {
    const shown = new DetailsShown();
    expect(shown.open('a')).toBe(true);
    shown.toggle('a');
    expect(shown.open('a')).toBe(false);
    expect(shown.open('b')).toBe(true);
    shown.mode = 'popover';
    expect(shown.open('b')).toBe(false);
    shown.toggle('b');
    expect(shown.open('b')).toBe(true);
    shown.mode = 'inline';
    shown.mode = 'popover';
    expect(shown.open('b')).toBe(false);
  });
});
