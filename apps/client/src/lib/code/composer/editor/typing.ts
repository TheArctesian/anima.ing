import { touchesChip, wholeChips } from './chips';
import { edit, type Edit, type Span } from './edit';
import type { Action, Page } from './keys/chords';
import { broken } from './keys/breaks';
import { codeUnwrap, fenceGuard } from './keys/code';
import { erased, inLine, unprefixed, type Reach } from './keys/erase';
import { toggled, wrapped } from './keys/pairs';
import { lines } from './markdown/blocks';
import type { Style } from './markdown/inline';

/** One input the browser is about to make: its `inputType`, its text, and the stretch it would change, if it says. */
export interface Typed {
  readonly type: string;
  readonly text: string | null;
  readonly target: Span | null;
}

/** The prompt as an input finds it, with its chips. */
export interface Desk extends Page {
  readonly chips: readonly Span[];
}

const FORMATS: Record<string, Style | undefined> = { formatBold: 'bold', formatItalic: 'italic', formatStrikeThrough: 'strike' };
const REACHES: readonly (readonly [string, Reach])[] = [
  ['Word', 'word'],
  ['SoftLine', 'line'],
  ['HardLine', 'line'],
];

/** The text an input puts in: over the selection, or over the stretch the browser names (a spelling correction). */
function inserted(typed: Typed, desk: Desk): Action {
  const text = (typed.text ?? '').replaceAll('\r\n', '\n').replaceAll('\r', '\n');
  const { sel, value, chips, mode } = desk;
  // A correction names its own stretch; only a key typed over the selection wraps it.
  const over = typed.target === null || (typed.target.start === sel.start && typed.target.end === sel.end);
  const pair = over && typed.type === 'insertText' && mode !== 'literal' && !touchesChip(sel, chips) ? wrapped(value, sel, text) : null;
  if (pair !== null) return { edit: pair, kind: 'other' };
  const span = wholeChips(typed.target ?? sel, chips);
  if (span.start === span.end && text === '') return null;
  return { edit: edit(span.start, span.end, text), kind: span.start === span.end && typed.type === 'insertText' ? 'insert' : 'other' };
}

/** Backspace and Delete at a bare caret, where a marker or a fence is not drawn (see `unprefixed`, `codeUnwrap`, `fenceGuard`). */
function guarded(desk: Desk, back: boolean, reach: Reach): Edit | null {
  if (desk.mode !== 'rich' || reach !== 'char') return null;
  const all = lines(desk.value, desk.mode);
  const caret = desk.sel.start;
  const marker = back ? (unprefixed(caret, all) ?? codeUnwrap(desk.value, caret, all)) : null;
  return marker ?? fenceGuard(desk.value, caret, all, back);
}

function deleted(typed: Typed, desk: Desk): Action {
  const { sel, value, chips } = desk;
  if (sel.start !== sel.end) {
    const span = wholeChips(sel, chips);
    return { edit: edit(span.start, span.end, ''), kind: 'delete' };
  }
  const back = !typed.type.endsWith('Forward');
  const reach = REACHES.find(([name]) => typed.type.includes(name))?.[1] ?? 'char';
  const guard = guarded(desk, back, reach);
  if (guard !== null) return { edit: guard, kind: 'other' };
  // A character is counted here, so an emoji goes whole; for a word or a line the browser knows the layout.
  const counted = reach === 'char' || typed.target === null || typed.target.start === typed.target.end;
  const span = wholeChips(counted ? erased(value, sel.start, back, reach) : inLine(value, sel.start, typed.target ?? sel, back), chips);
  return span.start === span.end ? null : { edit: edit(span.start, span.end, ''), kind: 'delete' };
}

/**
 * What an input does to the prompt (03, The editor: Keyboard behaviour). The editor makes every change itself and
 * draws the result, so the browser's own edit is always refused. Null: nothing happens.
 */
export function typedAction(typed: Typed, desk: Desk): Action {
  const { type } = typed;
  if (type === 'historyUndo') return 'undo';
  if (type === 'historyRedo') return 'redo';
  if (type === 'insertLineBreak' || type === 'insertParagraph') {
    const line = broken(desk.value, desk.sel, desk.mode);
    return { edit: line.edit, kind: line.plain ? 'insert' : 'other' };
  }
  if (type === 'insertText' || type === 'insertReplacementText' || type === 'insertFromYank') return inserted(typed, desk);
  if (type.startsWith('delete') && type !== 'deleteByDrag' && type !== 'deleteByCut') return deleted(typed, desk);
  const style = FORMATS[type];
  if (style === undefined || desk.mode !== 'rich') return null;
  const change = toggled(desk.value, desk.sel, style);
  return change === null ? null : { edit: change, kind: 'other' };
}

/** Pasted text of this many bytes or more is handed up, not put in the prompt (T3's `PASTED_TEXT_ATTACHMENT_THRESHOLD_BYTES`). */
export const LONG_PASTE_BYTES = 32 * 1024;
/** The most characters a prompt may hold (T3's `PROVIDER_SEND_TURN_MAX_INPUT_CHARS`). */
export const PROMPT_LIMIT = 120_000;

/** Whether pasted `text` is handed up: it is long, or the prompt of `room` characters around it would pass the limit (03, The editor: Pasting text). */
export function pasteIsLong(text: string, room: number): boolean {
  return new TextEncoder().encode(text).length >= LONG_PASTE_BYTES || room + text.length > PROMPT_LIMIT;
}
