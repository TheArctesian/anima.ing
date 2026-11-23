// The create-key form, checked before anything is sent (INT-003, D-23).
import type { HubError } from '$lib/hub/hub';
import type { ApiKeyInput } from './api';

const MINUTE_MS = 60_000;
const DAY_MS = 24 * 60 * MINUTE_MS;
/** The hub refuses an expiry more than 365 days after its own clock (D-23). */
const HUB_BOUND_MS = 365 * DAY_MS;
/** Request time and clock skew must not push a chosen key past the hub's 365 days. */
const BOUND_MARGIN_MS = 5 * MINUTE_MS;

const NAME_MAX = 64;

type ExpiryChoice = '30d' | '90d' | '1y' | 'date';

export const EXPIRY_CHOICES: readonly { readonly id: ExpiryChoice; readonly label: string }[] = [
  { id: '30d', label: '30 days' },
  { id: '90d', label: '90 days' },
  { id: '1y', label: '1 year' },
  { id: 'date', label: 'On a date' },
];

const PRESET_MS: Record<Exclude<ExpiryChoice, 'date'>, number> = {
  '30d': 30 * DAY_MS,
  '90d': 90 * DAY_MS,
  '1y': HUB_BOUND_MS - BOUND_MARGIN_MS,
};

export const SCOPES: readonly { readonly id: ApiKeyInput['scope']; readonly label: string; readonly hint: string }[] = [
  { id: 'READ', label: 'Read', hint: 'Queries only.' },
  { id: 'WRITE', label: 'Write', hint: 'Queries and changes.' },
];

/** The hub records each change made with an agent's key as Agent, and this cannot change later (LOG-015). */
export const MADE_FOR: readonly { readonly id: ApiKeyInput['madeFor']; readonly label: string }[] = [
  { id: 'USER', label: 'A script I run' },
  { id: 'AGENT', label: 'An AI agent' },
];

/** `madeFor` is `null` until the operator chooses: the form never guesses it (LOG-015). */
export interface NewKeyForm {
  name: string;
  scope: ApiKeyInput['scope'];
  madeFor: ApiKeyInput['madeFor'] | null;
  expiry: ExpiryChoice;
  date: string;
}

export function emptyForm(): NewKeyForm {
  return { name: '', scope: 'READ', madeFor: null, expiry: '90d', date: '' };
}

export const NAME_MISSING = 'Enter a name.';
export const NAME_TOO_LONG = `Use at most ${String(NAME_MAX)} characters.`;
export const MADE_FOR_MISSING = 'Choose who uses the key.';
export const DATE_MISSING = 'Choose an expiry date.';
export const DATE_PAST = 'Choose a date after today.';
export const DATE_TOO_FAR = 'Choose a date at most one year away.';

export interface NewKeyProblem {
  readonly field: 'name' | 'madeFor' | 'expiry';
  readonly message: string;
}

type NewKeyCheck = { readonly ok: true; readonly input: ApiKeyInput } | { readonly ok: false; readonly problems: readonly NewKeyProblem[] };

function startOfDay(text: string): Date | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(text);
  if (match === null) return null;
  const [year, month, day] = [Number(match[1]), Number(match[2]), Number(match[3])];
  const date = new Date(year, month - 1, day);
  const real = date.getFullYear() === year && date.getMonth() === month - 1 && date.getDate() === day;
  return real ? date : null;
}

export function latestExpiry(now: Date): Date {
  return new Date(now.getTime() + HUB_BOUND_MS - BOUND_MARGIN_MS);
}

/** A chosen date expires the key when that day begins, in this machine's time zone. */
export function expiryOf(form: Pick<NewKeyForm, 'expiry' | 'date'>, now: Date): Date | null {
  if (form.expiry === 'date') return startOfDay(form.date);
  return new Date(now.getTime() + PRESET_MS[form.expiry]);
}

export function dayText(date: Date): string {
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${String(date.getFullYear())}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

function nameProblem(name: string): NewKeyProblem | null {
  // Counted in code points, as the hub counts characters.
  const length = [...name].length;
  if (length === 0) return { field: 'name', message: NAME_MISSING };
  return length > NAME_MAX ? { field: 'name', message: NAME_TOO_LONG } : null;
}

function madeForProblem(madeFor: NewKeyForm['madeFor']): NewKeyProblem | null {
  return madeFor === null ? { field: 'madeFor', message: MADE_FOR_MISSING } : null;
}

function expiryProblem(expiresAt: Date | null, now: Date): NewKeyProblem | null {
  if (expiresAt === null) return { field: 'expiry', message: DATE_MISSING };
  if (expiresAt.getTime() <= now.getTime()) return { field: 'expiry', message: DATE_PAST };
  return expiresAt.getTime() > latestExpiry(now).getTime() ? { field: 'expiry', message: DATE_TOO_FAR } : null;
}

/** Nothing is sent while there is a problem (D-23). */
export function checkNewKey(form: NewKeyForm, now: Date): NewKeyCheck {
  const name = form.name.trim();
  const expiresAt = expiryOf(form, now);
  const problems = [nameProblem(name), madeForProblem(form.madeFor), expiryProblem(expiresAt, now)].filter((problem) => problem !== null);
  if (problems.length > 0 || expiresAt === null || form.madeFor === null) return { ok: false, problems };
  return { ok: true, input: { name, scope: form.scope, madeFor: form.madeFor, expiresAt: expiresAt.toISOString() } };
}

/** The hub's own text is not shown: a fixed sentence never carries anything the operator did not type (LOG-007). */
export function createFailure(error: HubError): string {
  switch (error.code) {
    case 'CONFLICT':
      return 'This name is taken.';
    case 'VALIDATION':
      return 'The hub refused the name or date.';
    case 'FORBIDDEN':
      return 'This sign-in cannot make API keys.';
    case 'unreachable':
      return 'The hub did not answer.';
    case 'malformed':
      return 'The hub sent an unreadable answer.';
    default:
      return 'The hub did not make the key.';
  }
}
