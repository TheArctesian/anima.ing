import type { Edit, Span } from '../edit';
import type { Style } from '../markdown/inline';

/** Typing one of these over a selection wraps it (03, The editor: Keyboard behaviour; T3's `SURROUND_CLOSE`). */
const CLOSERS = new Map([
  ['(', ')'],
  ['[', ']'],
  ['{', '}'],
  ["'", "'"],
  ['"', '"'],
  ['“', '”'],
  ['`', '`'],
  ['<', '>'],
  ['«', '»'],
  ['*', '*'],
  ['_', '_'],
]);

const DELIMITERS: Record<Style, string> = { bold: '**', italic: '*', strike: '~~', code: '`' };

/** The selection wrapped in `open` and its closing character, still selected; null when `open` opens no pair. */
export function wrapped(value: string, sel: Span, open: string): Edit | null {
  const close = CLOSERS.get(open);
  if (close === undefined || sel.start === sel.end) return null;
  return { ...sel, text: open + value.slice(sel.start, sel.end) + close, anchor: sel.start + open.length, focus: sel.end + open.length };
}

function runBefore(value: string, at: number, char: string): number {
  let count = 0;
  while (count < at && value[at - count - 1] === char) count += 1;
  return count;
}

function runAfter(value: string, at: number, char: string): number {
  let count = 0;
  while (value[at + count] === char) count += 1;
  return count;
}

/** Whether a run of `count` delimiter characters holds `style`: `***` is bold and italic, `**` is bold only. */
function holds(style: Style, count: number): boolean {
  if (style === 'italic') return count % 2 === 1;
  return count >= DELIMITERS[style].length;
}

/**
 * Mod-b, Mod-i, Mod-Shift-s and Mod-e (03, The editor: Rich features): the selection gets the style's delimiters, or
 * loses them when they are right around it or at its two ends. A bare caret gets an empty pair to type in. Null
 * across lines, where no inline style reaches.
 */
export function toggled(value: string, sel: Span, style: Style): Edit | null {
  const inner = value.slice(sel.start, sel.end);
  if (inner.includes('\n')) return null;
  const mark = DELIMITERS[style];
  const size = mark.length;
  const char = mark.charAt(0);
  const around = Math.min(runBefore(value, sel.start, char), runAfter(value, sel.end, char));
  if (holds(style, around)) {
    return { start: sel.start - size, end: sel.end + size, text: inner, anchor: sel.start - size, focus: sel.end - size };
  }
  const ends = Math.min(runAfter(inner, 0, char), runBefore(inner, inner.length, char));
  if (inner.length > 2 * ends && holds(style, ends)) {
    return { ...sel, text: inner.slice(size, -size), anchor: sel.start, focus: sel.end - 2 * size };
  }
  return { ...sel, text: mark + inner + mark, anchor: sel.start + size, focus: sel.end + size };
}
