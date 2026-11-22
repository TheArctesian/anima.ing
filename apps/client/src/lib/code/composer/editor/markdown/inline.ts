/** The inline styles the editor draws (03, The editor: Rich features). Links and underline are not styles. */
export type Style = 'bold' | 'italic' | 'strike' | 'code';

/** A stretch of a line that is drawn with `styles`. The delimiters around it belong to no piece. */
interface Piece {
  start: number;
  end: number;
  styles: readonly Style[];
}

/** One drawn stretch of a line: text, or a Markdown delimiter (`marker`). */
export interface Run {
  readonly start: number;
  readonly end: number;
  readonly styles: readonly Style[];
  readonly marker: boolean;
  /** The styled text this run is part of, delimiters included; null for plain text. Its markers show when the caret touches it. */
  readonly region: { readonly start: number; readonly end: number } | null;
}

interface Frame {
  readonly delimiter: string;
  readonly style: Style;
  readonly start: number;
  readonly pieces: Piece[];
}

interface Scan {
  readonly text: string;
  readonly literal: ReadonlySet<number>;
  readonly root: Piece[];
  readonly stack: Frame[];
}

/** A run of one delimiter character, and whether it may open or close a style where it stands. */
interface Delimiters {
  readonly char: string;
  readonly end: number;
  readonly canOpen: boolean;
  readonly canClose: boolean;
}

// Brackets, quotes, commas and Markdown delimiters end a URL or path; a lone `~` does not.
const PATH_TOKEN = /(?:[\p{L}\p{N}_/.:@%+=&?#-]|(?<!~)~(?!~))+/gu;
const FILE_EXTENSION = /^\.[a-z][a-z0-9]*(?::\d+){0,2}$/i;
// `_yes_/_no_`: each side of the slash is its own emphasis, so the word is not a path.
const EMPHASIS_ALTERNATIVES = /^_[^/]+_(?:\/_[^/]+_)+$/;
const WORD_BEFORE = /[\p{L}\p{M}\p{N}_]$/u;
const WORD_AFTER = /^[\p{L}\p{M}\p{N}_]/u;

/** The part of a token that may hold literal underscores, or null when the token is prose and not a path or file name. */
function pathBody(token: string): string | null {
  let end = token.length;
  while (end > 0 && '.:?'.includes(token.charAt(end - 1))) end -= 1;
  if (EMPHASIS_ALTERNATIVES.test(token.slice(0, end))) return null;
  while (end > 0 && token[end - 1] === '_') end -= 1;
  const body = token.slice(0, end);
  // A slash alone is prose (`and/_or_`); a path also has a dot, a scheme, a port or a root.
  const isPath = body.includes('/') ? /^~?\/|[.:]/.test(body) : FILE_EXTENSION.test(body.slice(body.lastIndexOf('.')));
  return isPath ? body : null;
}

/**
 * Where underscore runs start that belong to a URL, a path or a file name (`__init__.py`, `my_file.ts`) and so are
 * text, not emphasis (03, The editor: Markdown round trip).
 */
function literalUnderscores(text: string): Set<number> {
  const literal = new Set<number>();
  if (!text.includes('_')) return literal;
  for (const match of text.matchAll(PATH_TOKEN)) {
    const body = pathBody(match[0]);
    if (body === null) continue;
    let closesInside = false;
    let start = body.indexOf('_');
    while (start !== -1) {
      let end = start;
      while (body[end] === '_') end += 1;
      if (start > 0) {
        literal.add(match.index + start);
        closesInside ||= !WORD_AFTER.test(body.slice(end, end + 2));
      }
      start = body.indexOf('_', end);
    }
    // A leading run could only pair with a run inside the name, as in `__init__.py`.
    if (closesInside && body.startsWith('_')) literal.add(match.index);
  }
  return literal;
}

function sameStyles(left: readonly Style[], right: readonly Style[]): boolean {
  return left.length === right.length && left.every((style, index) => style === right[index]);
}

function push(scan: Scan, start: number, end: number, styles: readonly Style[] = []): void {
  if (end <= start) return;
  const pieces = scan.stack.at(-1)?.pieces ?? scan.root;
  const last = pieces.at(-1);
  if (last?.end === start && sameStyles(last.styles, styles)) last.end = end;
  else pieces.push({ start, end, styles });
}

/** One backtick pair on the line is inline code; any other run of backticks is text. */
function codeSpan(scan: Scan, index: number): number {
  const { text } = scan;
  let end = index;
  while (text[end] === '`') end += 1;
  const close = text.indexOf('`', end);
  if (end - index === 1 && close > index + 1 && text[close + 1] !== '`') {
    push(scan, index + 1, close, ['code']);
    return close + 1;
  }
  push(scan, index, end);
  return end;
}

function opensBoldInItalic(scan: Scan, index: number, run: Delimiters): boolean {
  const inItalic = scan.stack.at(-1)?.style === 'italic';
  return inItalic && run.end - index === 2 && run.canOpen && run.char !== '~' && !scan.stack.some((frame) => frame.style === 'bold');
}

function closes(scan: Scan, index: number, run: Delimiters): Frame | null {
  const top = scan.stack.at(-1);
  if (top === undefined || !run.canClose || opensBoldInItalic(scan, index, run)) return null;
  return scan.text.startsWith(top.delimiter, index) && index + top.delimiter.length <= run.end ? top : null;
}

/** Reads the delimiters of `run` from `index`: closes the open style, opens one, or leaves them as text. */
function delimiterStep(scan: Scan, index: number, run: Delimiters): number {
  const closed = closes(scan, index, run);
  if (closed !== null) {
    scan.stack.pop();
    for (const piece of closed.pieces) push(scan, piece.start, piece.end, [closed.style, ...piece.styles]);
    return index + closed.delimiter.length;
  }
  const left = run.end - index;
  if (!run.canOpen || (run.char === '~' && left < 2)) {
    push(scan, index, run.end);
    return run.end;
  }
  const length = run.char === '~' || left >= 2 ? 2 : 1;
  const style: Style = run.char === '~' ? 'strike' : length === 2 ? 'bold' : 'italic';
  // A style never nests in itself, so the stack is never deeper than the styles there are.
  if (scan.stack.some((frame) => frame.style === style)) {
    push(scan, index, run.end);
    return run.end;
  }
  scan.stack.push({ delimiter: run.char.repeat(length), style, start: index, pieces: [] });
  return index + length;
}

function delimiterRun(scan: Scan, start: number): number {
  const { text } = scan;
  const char = text.charAt(start);
  let end = start;
  while (text[end] === char) end += 1;
  if (scan.literal.has(start)) {
    push(scan, start, end);
    return end;
  }
  const before = text.charAt(start - 1);
  const after = text.charAt(end);
  // An underscore inside a word is text (`node_modules`); two code units, so a letter outside the BMP counts whole.
  const wordAfter = char === '_' && WORD_AFTER.test(text.slice(end, end + 2));
  const wordBefore = char === '_' && WORD_BEFORE.test(text.slice(Math.max(0, start - 2), start));
  const run: Delimiters = {
    char,
    end,
    canClose: before !== '' && !/\s/.test(before) && !wordAfter,
    canOpen: after !== '' && !/\s/.test(after) && !wordBefore,
  };
  let index = start;
  while (index < end) index = delimiterStep(scan, index, run);
  return end;
}

function step(scan: Scan, index: number): number {
  const char = scan.text[index];
  if (char === '\\') {
    // A backslash keeps the next character as text.
    push(scan, index, Math.min(index + 2, scan.text.length));
    return index + 2;
  }
  if (char === '`') return codeSpan(scan, index);
  if (char === '*' || char === '_' || char === '~') return delimiterRun(scan, index);
  push(scan, index, index + 1);
  return index + 1;
}

/** The styled pieces of one line, in order. Parsing only drops delimiters, so each piece keeps its place in the line. */
function pieces(text: string): Piece[] {
  const scan: Scan = { text, literal: literalUnderscores(text), root: [], stack: [] };
  let index = 0;
  while (index < text.length) index = step(scan, index);
  for (let frame = scan.stack.pop(); frame !== undefined; frame = scan.stack.pop()) {
    // A style that was opened and never closed is text, delimiter and all.
    push(scan, frame.start, frame.start + frame.delimiter.length);
    for (const piece of frame.pieces) push(scan, piece.start, piece.end, piece.styles);
  }
  return scan.root;
}

/**
 * One line of Markdown as runs of text and delimiters (03, The editor: Rich features). `**` and `__` are bold, `*`
 * and `_` italic, `~~` strike, one backtick pair code. Every character of the line is in exactly one run.
 */
export function runs(text: string): Run[] {
  const out: Run[] = [];
  let region: { start: number; end: number } | null = null;
  const add = (start: number, end: number, styles: readonly Style[], marker: boolean): void => {
    if (end <= start) return;
    const styled = marker || styles.length > 0;
    if (!styled) region = null;
    else if (region === null) region = { start, end };
    else region.end = end;
    out.push({ start, end, styles, marker, region: styled ? region : null });
  };
  let at = 0;
  for (const piece of pieces(text)) {
    add(at, piece.start, [], true);
    add(piece.start, piece.end, piece.styles, false);
    at = piece.end;
  }
  add(at, text.length, [], true);
  return out;
}
