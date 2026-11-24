/** The limit stays text so that "2.5" is reportable. */
export interface PolicyValues {
  /** Empty means no limit. */
  readonly sessionLimit: string;
}

export interface StoredPolicy {
  readonly sessionLimit: number | null;
}

export interface PolicyInput {
  sessionLimit?: number;
}

const MAX_SESSION_LIMIT = 100;

/** The one sentence for an invalid limit (RA-036). */
export const LIMIT_SENTENCE = 'Use a whole number from 1 to 100.';

export const DEFAULT_POLICY: PolicyValues = { sessionLimit: '' };

export function policyValues(stored: StoredPolicy): PolicyValues {
  return { sessionLimit: stored.sessionLimit === null ? '' : String(stored.sessionLimit) };
}

/** The limit typed, or null for none; undefined when the text is not a whole number. */
function typedLimit(text: string): number | null | undefined {
  const typed = text.trim();
  if (typed === '') return null;
  return /^\d{1,4}$/.test(typed) ? Number(typed) : undefined;
}

/** RA-036: a new device takes 1..100; an edit also takes 0, which clears the limit. */
export function limitProblem(values: PolicyValues, creating: boolean): string | null {
  const limit = typedLimit(values.sessionLimit);
  if (limit === null) return null;
  if (limit === undefined || limit > MAX_SESSION_LIMIT) return LIMIT_SENTENCE;
  return creating && limit === 0 ? LIMIT_SENTENCE : null;
}

/** RA-036: a new device sends only what differs from the hub's default (no limit). */
export function createPolicyInput(values: PolicyValues): PolicyInput {
  const limit = typedLimit(values.sessionLimit);
  return limit === null || limit === undefined ? {} : { sessionLimit: limit };
}

/** RA-036: a limit emptied or set to 0 is sent as 0, which clears a stored limit. */
export function updatePolicyInput(stored: PolicyValues, values: PolicyValues): PolicyInput {
  const before = typedLimit(stored.sessionLimit) ?? 0;
  const now = typedLimit(values.sessionLimit) ?? 0;
  return now === before ? {} : { sessionLimit: now };
}
