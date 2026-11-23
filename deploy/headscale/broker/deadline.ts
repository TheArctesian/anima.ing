/** A changing step that could not finish before the deadline is not started, so the 45 s limit never cuts a change (H1). */

import { StepError, type Runner } from './settle.ts';
import { LIST_STEPS, USERS_STEP, type Step } from './verbs.ts';

export const STEP_MS = 8_000;

const READS = new Set([...LIST_STEPS, USERS_STEP].map((step) => step.join(' ')));

export function changes(step: Step): boolean {
  return !READS.has(step.join(' '));
}

export function withDeadline(runner: Runner, deadline: number, clock: () => number): Runner {
  return (step) => {
    if (changes(step) && clock() + STEP_MS > deadline) return Promise.reject(new StepError('the request deadline is too near to start a change'));
    return runner(step);
  };
}
