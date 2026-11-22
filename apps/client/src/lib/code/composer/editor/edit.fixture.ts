import { applied, type Edit, type Span } from './edit';

/** Reads a prompt written with its selection: `ab|cd` is a caret, `a[bc]d` a selection. */
export function marked(text: string): { value: string; sel: Span; caret: number } {
  const caret = text.indexOf('|');
  if (caret !== -1) return { value: text.replace('|', ''), sel: { start: caret, end: caret }, caret };
  const start = text.indexOf('[');
  const end = text.indexOf(']') - 1;
  return { value: text.replace('[', '').replace(']', ''), sel: { start, end }, caret: end };
}

/** Writes a prompt with its selection, as `marked` reads it. */
export function shown(value: string, sel: { anchor: number; focus: number }): string {
  const start = Math.min(sel.anchor, sel.focus);
  const end = Math.max(sel.anchor, sel.focus);
  if (start === end) return `${value.slice(0, start)}|${value.slice(start)}`;
  return `${value.slice(0, start)}[${value.slice(start, end)}]${value.slice(end)}`;
}

/** The prompt after `change`, with its selection; `null` when there is no change. */
export function after(value: string, change: Edit | null): string | null {
  return change === null ? null : shown(applied(value, change), change);
}
