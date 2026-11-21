import type { Mode } from '../edit';

/** What one line of the prompt is. A fenced code block is a `fence` line, its `code` lines and its `close` line. */
export type Kind = 'text' | 'heading' | 'bullet' | 'ordered' | 'task' | 'quote' | 'rule' | 'fence' | 'code' | 'close';

/** One line of the prompt as the editor draws it (03, The editor: Rich features). */
export interface Line {
  readonly kind: Kind;
  /** Where the line starts in the prompt. */
  readonly start: number;
  readonly text: string;
  /** How many characters at the start of the line are the block's marker: `## `, `  - `, `> `, the fence. */
  readonly prefix: number;
  /** A heading's level, 1 to 6; else 0. */
  readonly level: number;
  /** How deep a list line is nested; 0 at the top. */
  readonly depth: number;
  /** An ordered line's marker as written (`3)`), a bullet's character. */
  readonly marker: string;
  readonly checked: boolean;
  /** For an ordered line: how many characters the widest marker of its list has, so the list's text starts at one column. */
  readonly gutter: number;
  /** For the lines of a code block: its opening fence, and the index of its `fence` line. */
  readonly fence: string;
  readonly block: number;
  /** The line starts, or ends, a list, a quote or a code block: the block's space goes above or below it. */
  readonly first: boolean;
  readonly last: boolean;
}

type Shape = Pick<Line, 'kind' | 'prefix' | 'level' | 'marker' | 'checked'> & { readonly indent: string | null };

const BARE = { kind: 'text', prefix: 0, level: 0, marker: '', checked: false } as const;
const TEXT: Shape = { ...BARE, indent: null };
const RULE = /^([-*_])(?:[ \t]*\1){2,}[ \t]*\r?$/;
const HEADING = /^(#{1,6})([ \t]+)/;
const QUOTE = /^>[ \t]*/;
const TASK = /^([ \t]*)-([ \t]+)\[([ xX])\]([ \t]*)/;
// A marker counts once a space follows it, so a lone `-` or `*` being typed stays text.
const ORDERED = /^([ \t]*)(\d+[.)])[ \t]+/;
const BULLET = /^([ \t]*)([-*+])[ \t]+/;

/** The fence and the language of a line that opens a code block: three or more backticks or tildes at its start. */
export function openingFence(line: string): { fence: string; language: string } | null {
  const match = /^(`{3,}|~{3,})([^\n]*)$/.exec(line);
  if (match === null) return null;
  const [, fence = '', language = ''] = match;
  return fence.startsWith('`') && language.includes('`') ? null : { fence, language };
}

/** Whether `line` closes a block opened with `fence`: the same character, at least as many, nothing after. */
export function closesFence(line: string, fence: string): boolean {
  const run = /^(`{3,}|~{3,})[ \t]*\r?$/.exec(line)?.[1];
  return run !== undefined && run[0] === fence[0] && run.length >= fence.length;
}

/** Where the character between the brackets of a task line is, or -1 when the line is no task. */
export function checkAt(line: string): number {
  const match = TASK.exec(line);
  if (match === null) return -1;
  const [whole, indent = '', space = '', , after = ''] = match;
  // Text right after the bracket, with no space, is no task (`- [x]y`).
  if (line.length > whole.length && after === '') return -1;
  return indent.length + 1 + space.length + 1;
}

function listShape(text: string): Shape | null {
  const check = checkAt(text);
  if (check !== -1) {
    const match = TASK.exec(text);
    return { kind: 'task', prefix: match?.[0].length ?? 0, level: 0, marker: '-', checked: text[check] !== ' ', indent: match?.[1] ?? '' };
  }
  const ordered = ORDERED.exec(text);
  if (ordered !== null) return { kind: 'ordered', prefix: ordered[0].length, level: 0, marker: ordered[2] ?? '', checked: false, indent: ordered[1] ?? '' };
  const bullet = BULLET.exec(text);
  if (bullet !== null) return { kind: 'bullet', prefix: bullet[0].length, level: 0, marker: bullet[2] ?? '', checked: false, indent: bullet[1] ?? '' };
  return null;
}

/** What a line outside a code block is. Block markers count only at the start of the line. */
function shape(text: string): Shape {
  if (RULE.test(text)) return { ...TEXT, kind: 'rule', prefix: text.length };
  const heading = HEADING.exec(text);
  if (heading !== null) return { ...TEXT, kind: 'heading', prefix: heading[0].length, level: heading[1]?.length ?? 1 };
  const quote = QUOTE.exec(text);
  if (quote !== null) return { ...TEXT, kind: 'quote', prefix: quote[0].length };
  return listShape(text) ?? TEXT;
}

/** How deep a list line with `indent` sits under the open lines above it; `open` keeps their indents. */
function depthOf(indent: string, open: string[]): number {
  for (let top = open.at(-1); top !== undefined; top = open.at(-1)) {
    if (top === indent) return open.length - 1;
    if (indent.length > top.length && indent.startsWith(top)) break;
    open.pop();
  }
  open.push(indent);
  return open.length - 1;
}

function family(line: Line | undefined): string {
  if (line === undefined) return '';
  if (line.block !== -1) return `code ${String(line.block)}`;
  if (line.kind === 'quote') return 'quote';
  return line.kind === 'bullet' || line.kind === 'ordered' || line.kind === 'task' ? 'list' : '';
}

/** Gives each ordered line the width of the widest marker at its depth in its list (T3's `data-marker-width`). */
function guttered(lines: readonly Line[]): Line[] {
  const out = [...lines];
  let widest = new Map<number, number>();
  let from = 0;
  for (let index = 0; index <= lines.length; index += 1) {
    const line = lines[index];
    if (line !== undefined && family(line) === 'list') {
      if (line.kind === 'ordered') widest.set(line.depth, Math.max(widest.get(line.depth) ?? 0, line.marker.length));
      continue;
    }
    for (let back = from; back < index; back += 1) {
      const listed = lines[back];
      if (listed?.kind === 'ordered') out[back] = { ...listed, gutter: widest.get(listed.depth) ?? 0 };
    }
    widest = new Map();
    from = index + 1;
  }
  return out;
}

/** Marks the lines that start and end a list (a nested one too), a quote or a code block. */
function edged(lines: readonly Line[]): Line[] {
  return lines.map((line, index) => {
    const own = family(line);
    if (own === '') return line;
    const above = lines[index - 1];
    const below = lines[index + 1];
    const nested = own === 'list';
    // An empty line under a list or a quote is its space already (T3 draws no line there, only the block's margin).
    const spaced = own !== 'list' && own !== 'quote' ? false : below?.kind === 'text' && below.text === '';
    return {
      ...line,
      first: family(above) !== own || (nested && line.depth > (above?.depth ?? 0)),
      last: !spaced && (family(below) !== own || (nested && line.depth > (below?.depth ?? 0))),
    };
  });
}

/**
 * The lines of the prompt and what each one is. Only `rich` reads Markdown blocks; `plain` and `literal` are text
 * lines (03, The editor: What it is). An opening fence with no closing one runs to the end, as Markdown has it.
 */
export function lines(markdown: string, mode: Mode): Line[] {
  const out: Line[] = [];
  const open: string[] = [];
  let block: { fence: string; at: number } | null = null;
  let start = 0;
  for (const text of markdown.split('\n')) {
    const base = { start, text, depth: 0, gutter: 0, fence: block?.fence ?? '', block: block?.at ?? -1, first: false, last: false };
    start += text.length + 1;
    if (block !== null) {
      const close = closesFence(text, block.fence);
      out.push({ ...BARE, ...base, kind: close ? 'close' : 'code', prefix: close ? text.length : 0 });
      if (close) block = null;
      continue;
    }
    const fence = mode === 'rich' ? openingFence(text) : null;
    if (fence !== null) {
      block = { fence: fence.fence, at: out.length };
      out.push({ ...BARE, ...base, kind: 'fence', prefix: fence.fence.length, fence: fence.fence, block: out.length });
      continue;
    }
    const { indent, ...found } = mode === 'rich' ? shape(text) : TEXT;
    if (indent === null) open.length = 0;
    out.push({ ...found, ...base, depth: indent === null ? 0 : depthOf(indent, open) });
  }
  return edged(guttered(out));
}

/** The index of the line that holds the offset `at`. */
export function lineIndexAt(all: readonly Line[], at: number): number {
  let low = 0;
  let high = all.length - 1;
  while (low < high) {
    const middle = Math.ceil((low + high) / 2);
    if ((all[middle]?.start ?? 0) <= at) low = middle;
    else high = middle - 1;
  }
  return low;
}

/** Where a line ends in the prompt, before its line break. */
export function lineEnd(line: Line): number {
  return line.start + line.text.length;
}
