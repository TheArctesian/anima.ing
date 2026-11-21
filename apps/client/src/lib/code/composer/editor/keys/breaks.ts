import { edit, type Edit, type Mode, type Span } from '../edit';
import { continued, indented, outdented } from '../lists/continuation';
import { lineEnd, lineIndexAt, lines, type Line } from '../markdown/blocks';
import { codeBreak, codeIndent } from './code';

/** Enter in a quote: a line with text is followed by a quote line; an empty one leaves the quote. */
function quoted(caret: number, line: Line): Edit | null {
  if (line.kind !== 'quote' || caret < line.start + line.prefix) return null;
  if (line.text.slice(line.prefix).trim() === '') return edit(line.start, lineEnd(line), '');
  return edit(caret, caret, `\n${line.text.slice(0, line.prefix)}`);
}

/**
 * Enter, when the composer did not take it (03, The editor: Keyboard behaviour): the rules of a code block, of a
 * list and of a quote, else a line break. A selection is replaced by a line break; `literal` text has no rules.
 * `plain` is true when the result is only a line break.
 */
export function broken(value: string, sel: Span, mode: Mode): { edit: Edit; plain: boolean } {
  const fallback = { edit: edit(sel.start, sel.end, '\n'), plain: true };
  if (sel.start !== sel.end || mode === 'literal') return fallback;
  const all = lines(value, mode);
  const line = all[lineIndexAt(all, sel.start)];
  if (line === undefined || line.kind === 'rule') return fallback;
  const rule = line.block === -1 ? (continued(value, sel.start) ?? quoted(sel.start, line)) : codeBreak(value, sel.start, all);
  return rule === null ? fallback : { edit: rule, plain: false };
}

/**
 * Tab and Shift-Tab (03, The editor: List continuation rules, Keyboard behaviour): in a code block the lines move;
 * on a list line with a bare caret the line nests or moves out. Null: the key moves focus, as in any text box.
 */
export function tabbed(value: string, sel: Span, mode: Mode, back: boolean): Edit | null {
  if (mode === 'literal') return null;
  const all = lines(value, mode);
  const line = all[lineIndexAt(all, sel.start)];
  if (line === undefined || line.kind === 'rule') return null;
  if (line.block !== -1) return codeIndent(value, sel, all, back ? 'out' : 'in');
  if (sel.start !== sel.end) return null;
  return back ? outdented(value, sel.start) : indented(value, sel.start);
}
