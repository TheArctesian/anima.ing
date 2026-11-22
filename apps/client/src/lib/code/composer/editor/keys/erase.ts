import { edit, lineAt, type Edit, type Span } from '../edit';
import { lineIndexAt, type Line } from '../markdown/blocks';

/** How far a delete key reaches from a bare caret. */
export type Reach = 'char' | 'word' | 'line';

const WORD_BACK = /(?:[\p{L}\p{M}\p{N}_]+|[^\s\p{L}\p{M}\p{N}_]+)?[ \t]*$/u;
const WORD_FORWARD = /^[ \t]*(?:[\p{L}\p{M}\p{N}_]+|[^\s\p{L}\p{M}\p{N}_]+)?/u;

/** The edges of what a reader sees as one character, so an emoji or a letter with its accent goes whole. */
function graphemeEdges(text: string): number[] {
  const edges = [0];
  for (const { segment } of new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(text)) {
    edges.push((edges.at(-1) ?? 0) + segment.length);
  }
  return edges;
}

function within(text: string, at: number, back: boolean, reach: Reach): number {
  if (reach === 'line') return back ? 0 : text.length;
  if (reach === 'word') {
    return back ? at - (WORD_BACK.exec(text.slice(0, at))?.[0].length ?? 0) : at + (WORD_FORWARD.exec(text.slice(at))?.[0].length ?? 0);
  }
  const edges = graphemeEdges(text);
  return back ? (edges.findLast((edge) => edge < at) ?? 0) : (edges.find((edge) => edge > at) ?? text.length);
}

/**
 * What a delete key takes from a bare caret: a character, a word or the rest of the line, never more than the line;
 * at the line's edge, the line break.
 */
export function erased(value: string, caret: number, back: boolean, reach: Reach): Span {
  const line = lineAt(value, caret);
  const edge = back ? line.start : line.end;
  if (caret === edge) return back ? { start: Math.max(0, caret - 1), end: caret } : { start: caret, end: Math.min(value.length, caret + 1) };
  const to = line.start + within(line.text, caret - line.start, back, reach);
  return back ? { start: to, end: caret } : { start: caret, end: to };
}

/** `target` kept inside the line of the caret, so a delete never takes hidden lines with it; at the line's edge, the line break. */
export function inLine(value: string, caret: number, target: Span, back: boolean): Span {
  const line = lineAt(value, caret);
  if (caret === (back ? line.start : line.end)) return erased(value, caret, back, 'char');
  return back ? { start: Math.max(line.start, Math.min(target.start, caret)), end: caret } : { start: caret, end: Math.min(line.end, Math.max(target.end, caret)) };
}

/**
 * Backspace where the text of a heading, a list line or a quote starts: the marker goes and the line is plain
 * text. The marker is not drawn, so this is how it is taken away.
 */
export function unprefixed(caret: number, all: readonly Line[]): Edit | null {
  const line = all[lineIndexAt(all, caret)];
  if (line === undefined || line.prefix === 0 || caret !== line.start + line.prefix) return null;
  if (line.kind === 'fence' || line.kind === 'close' || line.kind === 'rule') return null;
  return edit(line.start, caret, '');
}
