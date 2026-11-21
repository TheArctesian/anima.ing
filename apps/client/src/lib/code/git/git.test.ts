import { describe, expect, it } from 'vitest';
import { compact, elapsed, menuRows, quickAction, refusalWords, scopeLabel, type GitStatus } from './git';

const clean: GitStatus = {
  branch: 'anima/1234abcd',
  default_branch: 'main',
  is_default_branch: false,
  has_remote: true,
  upstream: 'origin/anima/1234abcd',
  ahead: 0,
  behind: 0,
  ahead_of_default: 0,
  files: [],
  additions: 0,
  deletions: 0
};
const file = { path: 'a.ts', kind: 'modified', additions: 2, deletions: 1 } as const;
const dirty: GitStatus = { ...clean, files: [file], additions: 2, deletions: 1 };
const without = (status: GitStatus, key: 'branch' | 'upstream'): GitStatus => Object.fromEntries(Object.entries(status).filter(([name]) => name !== key)) as unknown as GitStatus;

describe('the main Git action of the details card', () => {
  it('follows the order of T3 Code for each state of a branch', () => {
    const cases: readonly [string, GitStatus | undefined, boolean, string, number][] = [
      ['an action runs', dirty, true, 'Commit', 0],
      ['no status', undefined, false, 'Commit', 0],
      ['detached', without(dirty, 'branch'), false, 'Commit', 0],
      ['dirty', dirty, false, 'Commit, push & PR', 3],
      ['dirty and behind', { ...dirty, behind: 1 }, false, 'Commit', 1],
      ['dirty, no remote', { ...dirty, has_remote: false }, false, 'Commit', 1],
      ['dirty on the default branch', { ...dirty, is_default_branch: true }, false, 'Commit & push', 2],
      ['never pushed', { ...without(clean, 'upstream'), ahead_of_default: 2 }, false, 'Push & create PR', 2],
      ['never pushed, nothing new', without(clean, 'upstream'), false, 'Push', 0],
      ['diverged', { ...clean, ahead: 1, behind: 1 }, false, 'Sync ref', 0],
      ['behind', { ...clean, behind: 2 }, false, 'Pull', 0],
      ['ahead', { ...clean, ahead: 1, ahead_of_default: 1 }, false, 'Push & create PR', 2],
      ['pushed, no pull request', { ...clean, ahead_of_default: 1 }, false, 'Create PR', 1],
      ['up to date', clean, false, 'Commit', 0]
    ];
    for (const [name, status, busy, label, steps] of cases) {
      const action = quickAction(status, busy);
      expect([name, action.label, action.steps.length]).toEqual([name, label, steps]);
      expect(action.hint === undefined, name).toBe(steps > 0);
    }
  });

  it('gives a reason for each menu row that is off', () => {
    expect(menuRows(undefined)).toEqual([]);
    expect(menuRows({ ...dirty, has_remote: false }).map((row) => row.step)).toEqual(['commit']);
    const rows = menuRows(clean);
    expect(rows.map((row) => row.label)).toEqual(['Commit', 'Push', 'Create PR']);
    expect(rows.every((row) => row.hint !== undefined)).toBe(true);
    const ready = menuRows({ ...dirty, ahead: 1, ahead_of_default: 1 });
    expect(ready.every((row) => row.hint === undefined)).toBe(true);
    expect(menuRows({ ...clean, is_default_branch: true, ahead: 1 })[2]?.hint).toContain('default branch');
    expect(menuRows({ ...clean, push_url: 'https://github.com/ada/engine.git' })[1]?.detail).toBe('To https://github.com/ada/engine.git');
  });
});

describe('the words and the counts', () => {
  it('tells a person that an agent cannot do their Git actions', () => {
    expect(refusalWords('from_an_agent')).toContain('Only you');
    expect(refusalWords('running')).toBe('Interrupt the current turn before reverting checkpoints.');
  });

  it('labels a scope, a large count and a running time', () => {
    expect([scopeLabel({ scope: 'uncommitted' }), scopeLabel({ scope: 'thread' }), scopeLabel({ scope: 'turn', turn: 3 })]).toEqual(['Uncommitted', 'Changes', 'Turn 3']);
    expect([compact(950), compact(1200), compact(15_400), compact(3_000_000)]).toEqual(['950', '1.2k', '15k', '3m']);
    expect([elapsed(8.9), elapsed(72)]).toEqual(['8s', '1m 12s']);
  });
});
