import { describe, expect, it } from 'vitest';
import { ITEMS, atDefault, densityFor, fits, keepChoices, shows, storedChoices, toggled } from './shown';
import { branchLabel, checksCount, ended, matchingRefs, offersNewRef, pullRequestAction, pullRequestName, refBadge, relatedTrail, runningHeading, scheduleLine, scriptLabel, serverLine, workspaceLabel } from './labels';
import { contextTone, elapsed, limitTone, percent, shortTokens, untilReset } from './meters/format';

describe('the items of the card', () => {
  it('shows ten items by default and keeps the two meters off', () => {
    expect(ITEMS.filter((item) => shows({}, item.id)).map((item) => item.id)).not.toContain('usage');
    expect(shows({}, 'context')).toBe(false);
    expect(shows({}, 'branch')).toBe(true);
  });

  it('keeps only a choice that differs from the default', () => {
    const off = toggled({}, 'servers');
    expect(off).toEqual({ servers: false });
    expect(toggled(off, 'servers')).toEqual({});
    expect(atDefault(toggled({}, 'usage'))).toBe(false);
  });

  it('reads back what it kept and drops what is not an item', () => {
    const kept = new Map<string, string>();
    const store = { getItem: (key: string) => kept.get(key) ?? null, setItem: (key: string, value: string) => void kept.set(key, value) };
    keepChoices(store, { usage: true });
    expect(storedChoices(store)).toEqual({ usage: true });
    kept.set('anima.code.details-items', '{"usage":"yes","other":true}');
    expect(storedChoices(store)).toEqual({});
    kept.set('anima.code.details-items', 'not json');
    expect(storedChoices(store)).toEqual({});
  });

  it('picks the richest density that fits and hides the parts of a lower one', () => {
    const heights = { full: 500, compact: 300, essential: 200 };
    expect(densityFor(600, heights)).toBe('full');
    expect(densityFor(400, heights)).toBe('compact');
    expect(densityFor(100, heights)).toBe('essential');
    expect(fits('compact', 'runContext')).toBe(false);
    expect(fits('compact', 'openIn')).toBe(true);
    expect(fits('essential', 'openIn')).toBe(false);
    expect(fits('essential', 'meters')).toBe(true);
  });
});

describe('the words of the card', () => {
  it('names the workspace by its mode, its worktree and its lock', () => {
    expect(workspaceLabel({ mode: 'worktree', locked: false })).toBe('New worktree');
    expect(workspaceLabel({ mode: 'local', locked: false })).toBe('Current checkout');
    expect(workspaceLabel({ mode: 'local', worktreePath: '/w/a', locked: false })).toBe('Current worktree');
    expect(workspaceLabel({ mode: 'local', worktreePath: '/w/fix-login/', locked: true })).toBe('fix-login');
    expect(workspaceLabel({ mode: 'local', locked: true })).toBe('Local checkout');
  });

  it('names the branch row and marks a ref with one word', () => {
    expect(branchLabel({})).toBe('Select ref');
    expect(branchLabel({ name: 'main', base: true, fromRemote: true })).toBe('From main (remote)');
    expect(branchLabel({ name: 'fix' })).toBe('fix');
    expect(refBadge({ name: 'a', current: true, remote: true })).toBe('current');
    expect(refBadge({ name: 'a', remote: true, isDefault: true })).toBe('remote');
    expect(refBadge({ name: 'a' })).toBeUndefined();
  });

  it('filters refs by a part of the name and offers a new ref for a name that is free', () => {
    const refs = [{ name: 'main' }, { name: 'fix/login' }];
    expect(matchingRefs(refs, 'LOG').map((ref) => ref.name)).toEqual(['fix/login']);
    expect(offersNewRef(refs, 'main')).toBe(false);
    expect(offersNewRef(refs, 'new')).toBe(true);
    expect(offersNewRef(refs, ' ')).toBe(false);
  });

  it('picks one action of a pull request in the order of T3 Code', () => {
    expect(pullRequestAction({ number: 1, state: 'open', conflicting: true, canMerge: true })).toBe('resolve');
    expect(pullRequestAction({ number: 1, state: 'draft', canMarkReady: true })).toBe('ready');
    expect(pullRequestAction({ number: 1, state: 'open', checks: { done: 3, total: 3, failed: 1 }, canMerge: true })).toBe('fix');
    expect(pullRequestAction({ number: 1, state: 'open', checks: { done: 1, total: 3, failed: 0 }, canMerge: true })).toBeUndefined();
    expect(pullRequestAction({ number: 1, state: 'open', checks: { done: 3, total: 3, failed: 0 }, canMerge: true })).toBe('merge');
    expect(pullRequestAction({ number: 1, state: 'merged', canMerge: true })).toBeUndefined();
  });

  it('says a pull request, its checks, a script, a server and an automation in words', () => {
    expect(pullRequestName({ number: 12, state: 'open', title: 'Fix it' })).toBe('Pull request #12, open: Fix it');
    expect(pullRequestName({ number: 12 })).toBe('Pull request #12');
    expect(checksCount({ done: 2, total: 5, failed: 1 })).toBe('2/5 checks');
    expect(checksCount({ done: 5, total: 5, failed: 2 })).toBe('2 failed');
    expect(scriptLabel({ id: 's', name: 'dev', icon: 'play', onSetup: true, onSettle: true })).toBe('dev (setup, on settle)');
    expect(serverLine({ id: 's', port: 5173, startedBy: 'agent', process: 'vite', forwardedTo: 5173 })).toBe('Forwarded to :5173 · Agent · vite');
    expect(scheduleLine({ id: 'a', title: 't', schedule: 'Daily', enabled: false, lastRun: 'never' })).toBe('Daily · paused');
    expect(scheduleLine({ id: 'a', title: 't', schedule: 'Daily', enabled: true, lastRun: 'never', next: 'in 2 hours' })).toBe('Daily · next in 2 hours');
  });

  it('sorts subagents by their state and says how many run', () => {
    const agent = { id: 'a', kind: 'subagent', title: 'Tests', status: 'running', seconds: 75 } as const;
    expect(ended(agent)).toBe(false);
    expect(ended({ ...agent, status: 'failed' })).toBe(true);
    expect(relatedTrail(agent)).toBe('1m 15s');
    expect(relatedTrail({ ...agent, status: 'failed' })).toBe('Failed');
    expect(relatedTrail({ ...agent, machine: 'nas' })).toBe('on nas · 1m 15s');
    expect(runningHeading('Lineage', [agent])).toBe('Lineage · 1 running');
    expect(runningHeading('Lineage', [])).toBe('Lineage');
  });
});

describe('the numbers of the card', () => {
  it('writes tokens in short form', () => {
    expect([950, 9500, 9000, 128_400, 1_200_000, 0].map(shortTokens)).toEqual(['950', '9.5k', '9k', '128k', '1.2m', '0']);
    expect(shortTokens(null)).toBe('0');
  });

  it('writes a time to a reset and a time of a run', () => {
    expect([50 * 3600, 3 * 3600 + 20 * 60, 45 * 60].map(untilReset)).toEqual(['2d 2h', '3h 20m', '45m']);
    expect([42, 125].map(elapsed)).toEqual(['42s', '2m 5s']);
  });

  it('warns when a limit is nearly used and when the context is nearly full', () => {
    expect([50, 20, 10].map(limitTone)).toEqual(['plain', 'warning', 'error']);
    expect([50, 80, 90].map(contextTone)).toEqual(['plain', 'warning', 'error']);
    expect(percent(50, 200)).toBe(25);
    expect(percent(5, 0)).toBe(0);
  });
});
