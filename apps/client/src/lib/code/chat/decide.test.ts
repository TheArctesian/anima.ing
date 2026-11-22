import { describe, expect, it } from 'vitest';
import { DECLINED } from '$lib/agent-threads/live/drive';
import type { Ask } from '$lib/agent-threads/live/run';
import { decided } from './decide';

const ask = (sessionRules: readonly string[]): Ask => ({ kind: 'command', tool: 'Bash', subject: 'bun test', diff: null, sessionRules, questions: [] });

describe('decided', () => {
  it('allows once on Approve and declines on Decline', () => {
    expect(decided('accept', ask([]))).toEqual({ answer: { answer: 'allow' }, interrupt: false });
    expect(decided('decline', ask([]))).toEqual({ answer: { answer: 'deny', message: DECLINED }, interrupt: false });
  });

  it('allows for the session by the first rule, and once when the runner offered no rule', () => {
    expect(decided('acceptForSession', ask(['Bash(bun test:*)'])).answer).toEqual({ answer: 'allow_for_session', rule: 0 });
    expect(decided('acceptForSession', ask([])).answer).toEqual({ answer: 'allow' });
  });

  it('declines and stops the turn on Cancel', () => {
    expect(decided('cancel', ask([]))).toEqual({ answer: { answer: 'deny', message: DECLINED }, interrupt: true });
  });
});
