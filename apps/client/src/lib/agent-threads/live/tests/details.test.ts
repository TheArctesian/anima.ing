import { describe, expect, it } from 'vitest';
import { runRowFrom } from '../run';
import { usageFrom, workspaceFrom } from '../details';
import { rowFrame } from './frames.fixture';

describe('the facts of the details card on a row (TH112)', () => {
  it('reads the workspace and the use of tokens, and leaves out a part that does not read', () => {
    const row = runRowFrom(
      rowFrame('idle', {
        workspace: { kind: 'worktree', top: '/w/side' },
        usage: { context: 10, window: 200000, input: 10, output: 5, cost_usd: 0.25 },
      }),
    );
    expect(row?.workspace).toEqual({ kind: 'worktree', top: '/w/side' });
    expect(row?.usage).toEqual({ context: 10, window: 200000, input: 10, output: 5, costUsd: 0.25 });
    expect(runRowFrom(rowFrame('idle'))?.workspace).toBeNull();
    expect(workspaceFrom({ kind: 'svn', top: '/w' })).toBeNull();
    expect(usageFrom({ input: -1, output: 5 })).toBeNull();
    expect(usageFrom({ input: 1, output: 2 })).toEqual({ context: null, window: null, input: 1, output: 2, costUsd: null });
  });
});
