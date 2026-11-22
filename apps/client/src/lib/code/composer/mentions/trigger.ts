/**
 * Which list the caret asks for (03 "Triggers"; T3 Code `packages/shared/src/composerTrigger.ts`). The token is the
 * text from the last white space up to the caret; its first character names the list.
 */
export type TriggerKind = 'path' | 'pull-request' | 'slash-command' | 'skill';

export interface Trigger {
  readonly kind: TriggerKind;
  /** The token without its trigger character. */
  readonly query: string;
  /** Offsets in the prompt: the range a choice replaces, from the trigger character to the caret. */
  readonly start: number;
  readonly end: number;
}

const CURRENCY_START = /^\p{Sc}/u;
/** `$5`, `€9.99`, `$10k` and `$10k.` are money, not skills. */
const MONEY = /^\.?[0-9][0-9,._]*(?:[kKmMbBtT]|[eE][0-9]+)?[.,;:!?)]*$/;
/** `#`, then letters, digits, `_` or `-`, at the token start or right after an opening bracket. */
const PULL_REQUEST = /(?:^|\p{Ps})#([\p{L}\p{N}][\p{L}\p{N}_-]*)?$/u;

function isSpace(char: string | undefined): boolean {
  return char === ' ' || char === '\n' || char === '\t' || char === '\r';
}

function tokenStart(prompt: string, caret: number): number {
  let index = caret - 1;
  while (index >= 0 && !isSpace(prompt[index])) index -= 1;
  return index + 1;
}

/** A second `/` or a `\` anywhere in the word makes it a path such as `/src/a.ts`, also after the caret. */
function isPathWord(prompt: string, start: number, caret: number): boolean {
  let end = caret;
  while (end < prompt.length && !isSpace(prompt[end])) end += 1;
  return /[/\\]/.test(prompt.slice(start + 1, end));
}

function pullRequestAt(token: string, caret: number): Trigger | null {
  if (/^[@\p{Sc}]/u.test(token)) return null;
  const match = PULL_REQUEST.exec(token);
  if (match === null) return null;
  const query = match[1] ?? '';
  return { kind: 'pull-request', query, start: caret - query.length - 1, end: caret };
}

/** The trigger the caret stands in, or null. The order of the tests is the source's: `/`, `#`, a currency sign, `@`. */
export function triggerAt(prompt: string, caret: number): Trigger | null {
  const end = Number.isFinite(caret) ? Math.max(0, Math.min(prompt.length, Math.floor(caret))) : prompt.length;
  const start = tokenStart(prompt, end);
  const token = prompt.slice(start, end);
  if (token.startsWith('/')) {
    return isPathWord(prompt, start, end) ? null : { kind: 'slash-command', query: token.slice(1), start, end };
  }
  const pullRequest = pullRequestAt(token, end);
  if (pullRequest !== null) return pullRequest;
  const currency = CURRENCY_START.exec(token);
  if (currency !== null) {
    const query = token.slice(currency[0].length);
    return MONEY.test(query) ? null : { kind: 'skill', query, start, end };
  }
  return token.startsWith('@') ? { kind: 'path', query: token.slice(1), start, end } : null;
}

/**
 * Which trigger's menu is open and which one Escape closed (03 "Triggers", open and close rules; T3 Code
 * `useComposerTriggerState.ts`). A closed trigger stays closed while its kind and start stay the same.
 */
export interface TriggerWatch {
  readonly open: Trigger | null;
  readonly dismissed: Trigger | null;
}

export const NO_TRIGGER: TriggerWatch = { open: null, dismissed: null };

function same(a: Trigger | null, b: Trigger | null): boolean {
  return a !== null && b !== null && a.kind === b.kind && a.start === b.start;
}

/** After the text changed: the trigger at the caret opens, unless Escape closed that same trigger. */
export function afterEdit(watch: TriggerWatch, at: Trigger | null): TriggerWatch {
  return same(watch.dismissed, at) ? { open: null, dismissed: watch.dismissed } : { open: at, dismissed: null };
}

/** After the caret moved with no edit: the menu stays only for the same trigger; a move never opens one. */
export function afterCaretMove(watch: TriggerWatch, at: Trigger | null): TriggerWatch {
  return { open: same(watch.open, at) ? at : null, dismissed: same(watch.dismissed, at) ? watch.dismissed : null };
}

/** After Escape: the trigger at the caret is closed and remembered. */
export function afterEscape(at: Trigger | null): TriggerWatch {
  return { open: null, dismissed: at };
}

/** After a reset, such as a loaded draft: the trigger at the caret may open again. */
export function afterReset(at: Trigger | null): TriggerWatch {
  return { open: at, dismissed: null };
}
