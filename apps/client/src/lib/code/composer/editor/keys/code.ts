import { edit, lineAt, type Edit, type Span } from '../edit';
import { lineEnd, lineIndexAt, type Line } from '../markdown/blocks';

/** Tab in a code block moves lines by two spaces (T3's `CODE_BLOCK_INDENT`). */
const INDENT = '  ';

/** The closing line of the code block that `line` is in, or null while the block is open to the end. */
function closeOf(all: readonly Line[], line: Line): Line | null {
  for (let index = line.block + 1; all[index]?.block === line.block; index += 1) {
    if (all[index]?.kind === 'close') return all[index] ?? null;
  }
  return null;
}

/** Enter at the end of an opening fence with no closing one: the block is closed and the caret is on its first line. */
function opened(caret: number, line: Line, all: readonly Line[]): Edit | null {
  if (caret !== lineEnd(line) || closeOf(all, line) !== null) return null;
  return edit(caret, caret, `\n\n${line.fence}`, caret + 1);
}

/** Enter after two blank lines at the end of the code: they go, and the caret is on a new line under the block. */
function exited(value: string, caret: number, line: Line, all: readonly Line[]): Edit | null {
  const close = closeOf(all, line);
  const codeStart = lineEnd(all[line.block] ?? line) + 1;
  const codeEnd = close === null ? value.length : close.start - 1;
  const blank = caret === codeEnd ? /\n[ \t]*\n[ \t]*$/.exec(value.slice(codeStart, caret)) : null;
  if (blank === null) return null;
  // An open block gets its closing fence, so the text after it is not read as code.
  return edit(caret - blank[0].length, close === null ? caret : lineEnd(close), `\n${close?.text ?? line.fence}\n`);
}

/**
 * Enter in a code block (03, The editor: Keyboard behaviour): an opening fence gets its block; two blank lines at
 * the end leave the block; else the new line keeps the leading white space of this one. Null: a plain line break.
 */
export function codeBreak(value: string, caret: number, all: readonly Line[]): Edit | null {
  const line = all[lineIndexAt(all, caret)];
  if (line === undefined) return null;
  if (line.kind === 'fence') return opened(caret, line, all);
  if (line.kind !== 'code') return null;
  const exit = exited(value, caret, line, all);
  if (exit !== null) return exit;
  const indent = /^[ \t]*/.exec(value.slice(line.start, caret))?.[0] ?? '';
  return indent === '' ? null : edit(caret, caret, `\n${indent}`);
}

function outdent(line: string): string {
  if (line.startsWith(INDENT)) return line.slice(INDENT.length);
  return line.startsWith('\t') || line.startsWith(' ') ? line.slice(1) : line;
}

/**
 * Tab and Shift-Tab in a code block (03, The editor: Keyboard behaviour): the selected lines move in or out by two
 * spaces; a bare caret gets two spaces. Null when the selection is not inside the code of one block.
 */
export function codeIndent(value: string, sel: Span, all: readonly Line[], direction: 'in' | 'out'): Edit | null {
  const first = all[lineIndexAt(all, sel.start)];
  const last = all[lineIndexAt(all, sel.end)];
  if (first?.kind !== 'code' || last?.kind !== 'code' || first.block !== last.block) return null;
  if (sel.start === sel.end && direction === 'in') return edit(sel.start, sel.start, INDENT);
  // A selection that ends at the start of a line does not take that line.
  const end = lineAt(value, Math.max(sel.start, sel.end - 1)).end;
  const before = value.slice(first.start, end).split('\n');
  const after = before.map((line) => (direction === 'out' ? outdent(line) : line === '' ? line : INDENT + line));
  const text = after.join('\n');
  // Nothing to move: the key is not the block's, so Shift-Tab can leave the editor.
  if (text === before.join('\n')) return null;
  const anchor = Math.max(first.start, sel.start + (after[0]?.length ?? 0) - (before[0]?.length ?? 0));
  const focus = Math.max(anchor, sel.end + text.length - (end - first.start));
  return { start: first.start, end, text, anchor, focus };
}

/** Backspace at the start of a code block: its fences go and its lines are plain lines (03, The editor: Keyboard behaviour). */
export function codeUnwrap(value: string, caret: number, all: readonly Line[]): Edit | null {
  const index = lineIndexAt(all, caret);
  const line = all[index];
  if (line?.kind !== 'code' || caret !== line.start || index !== line.block + 1) return null;
  const open = all[line.block];
  const close = closeOf(all, line);
  if (open === undefined) return null;
  const code = value.slice(line.start, close === null ? value.length : close.start - 1);
  return edit(open.start, close === null ? value.length : lineEnd(close), code, open.start);
}

/**
 * The delete keys at the hidden closing fence, where a plain delete would join text to the fence and open the block
 * again: Backspace under it takes an empty line away, or only moves the caret onto the fence; Delete above it does
 * nothing. Null anywhere else.
 */
export function fenceGuard(value: string, caret: number, all: readonly Line[], back: boolean): Edit | null {
  const index = lineIndexAt(all, caret);
  const line = all[index];
  const close = all[back ? index - 1 : index + 1];
  if (line === undefined || close?.kind !== 'close') return null;
  if (!back) return caret === lineEnd(line) ? edit(caret, caret, '') : null;
  if (caret !== line.start) return null;
  return line.text === '' ? edit(lineEnd(close), caret, '') : edit(caret, caret, '', lineEnd(close));
}
