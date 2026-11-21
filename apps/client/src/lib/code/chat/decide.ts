import { DECLINED } from '$lib/agent-threads/live/drive';
import type { Answer, Ask } from '$lib/agent-threads/live/run';

/** What the page sends for one answer of the approval row: the answer, and whether the turn stops after it. */
export interface Decided {
  readonly answer: Answer;
  readonly interrupt: boolean;
}

/**
 * The runner's answer for a decision of the timeline's approval row ("accept", "decline", or a choice of its "More"
 * menu). "Always allow this session" allows by the first session rule the runner offered, and once when it offered none;
 * "Cancel" declines and stops the turn, as in T3.
 */
export function decided(decision: string, ask: Ask): Decided {
  switch (decision) {
    case 'accept':
      return { answer: { answer: 'allow' }, interrupt: false };
    case 'acceptForSession':
      return { answer: ask.sessionRules.length > 0 ? { answer: 'allow_for_session', rule: 0 } : { answer: 'allow' }, interrupt: false };
    case 'cancel':
      return { answer: { answer: 'deny', message: DECLINED }, interrupt: true };
    default:
      return { answer: { answer: 'deny', message: DECLINED }, interrupt: false };
  }
}
