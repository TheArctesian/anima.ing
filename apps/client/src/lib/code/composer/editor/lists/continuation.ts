import { edit, lineAt, type Edit, type Span } from '../edit';
import { indentWidth, listMarker, nextMarker, nextNumber, type ListMarker, type ParsedMarker } from './markers';

type Ordered = Extract<ListMarker, { kind: 'ordered' }>;

/**
 * The rewrite of the ordered lines under `after` so they count on from `marker` plus two: the line between is the
 * one being added (03, The editor: List continuation rules). Null when no number changes.
 */
function renumbered(value: string, after: number, marker: Ordered): { end: number; text: string } | null {
  const base = indentWidth(marker.indent);
  const parts: string[] = [];
  let expected = Number.parseInt(marker.numberText, 10) + 2;
  let position = after;
  let copied = after;
  while (position < value.length && Number.isSafeInteger(expected)) {
    const line = lineAt(value, position + 1);
    position = line.end;
    const width = indentWidth(line.text);
    if (line.text.trim() === '' || width < base) break;
    if (width > base) continue;
    const found = listMarker(line.text)?.marker;
    if (found?.kind !== 'ordered' || found.delimiter !== marker.delimiter) break;
    const numberStart = line.start + found.indent.length;
    const next = found.numberText.startsWith('0') ? String(expected).padStart(found.numberText.length, '0') : String(expected);
    if (next !== found.numberText) {
      parts.push(value.slice(copied, numberStart), next);
      copied = numberStart + found.numberText.length;
    }
    expected += 1;
  }
  return parts.length === 0 ? null : { end: copied, text: parts.join('') };
}

/** The list lines right above the line that starts at `lineStart`, nearest first, up to the first line that is no list line. */
function* markersAbove(value: string, lineStart: number): Generator<ParsedMarker> {
  let start = lineStart;
  while (start > 0) {
    const line = lineAt(value, start - 1);
    const parsed = listMarker(line.text);
    if (parsed === null) return;
    yield parsed;
    start = line.start;
  }
}

function parentOf(value: string, lineStart: number, marker: ListMarker): ListMarker | null {
  const width = indentWidth(marker.indent);
  for (const above of markersAbove(value, lineStart)) {
    if (indentWidth(above.marker.indent) < width) return above.marker;
  }
  return null;
}

/** The number a line takes when it moves under `before`: it counts on from a sibling, else it starts at 1. */
function nestedNumber(marker: ListMarker, before: ListMarker | undefined): { replaced: number; text: string } {
  if (marker.kind !== 'ordered') return { replaced: 0, text: '' };
  const continues = before?.kind === 'ordered' && before.delimiter === marker.delimiter;
  return { replaced: marker.numberText.length, text: continues ? nextNumber(before.numberText) : '1' };
}

/** Enter on an empty list line: the marker goes; a nested line becomes a line of the list above it. */
function emptied(value: string, caret: number, line: Span, parsed: ParsedMarker): Edit {
  const parent = parentOf(value, line.start, parsed.marker);
  const text = parent === null ? '' : nextMarker(parent);
  const end = Math.max(caret, line.start + parsed.end);
  const below = parent?.kind === 'ordered' ? renumbered(value, line.end, parent) : null;
  if (below === null) return edit(line.start, end, text);
  return edit(line.start, below.end, text + value.slice(end, line.end) + below.text, line.start + text.length);
}

/**
 * Enter in a list line (03, The editor: List continuation rules). With text: a new line with the next marker, and
 * the ordered lines below count on. Empty: see `emptied`. Null when the caret is not in the content of a list line.
 */
export function continued(value: string, caret: number): Edit | null {
  const line = lineAt(value, caret);
  const parsed = listMarker(line.text);
  if (parsed === null || parsed.marker.space === '') return null;
  const markerEnd = line.start + parsed.end;
  if (caret < markerEnd) return null;
  if (value.slice(markerEnd, line.end).trim() === '') return emptied(value, caret, line, parsed);
  const text = `\n${nextMarker(parsed.marker)}`;
  const below = parsed.marker.kind === 'ordered' ? renumbered(value, line.end, parsed.marker) : null;
  if (below === null) return edit(caret, caret, text);
  return edit(caret, below.end, text + value.slice(caret, line.end) + below.text, caret + text.length);
}

/** The caret after the start of the line at `lineStart` was rewritten, up to `replacedEnd`, to `indent` and a number. */
function caretAfter(caret: number, lineStart: number, replacedEnd: number, start: { indent: string; text: string }): number {
  return caret < replacedEnd ? lineStart + start.indent.length : caret - replacedEnd + lineStart + start.text.length;
}

/**
 * Tab on a list line: it nests under the line above, at that line's content column, and an ordered number starts
 * again. With no line above to nest under, two spaces (03, The editor: List continuation rules).
 */
export function indented(value: string, caret: number): Edit | null {
  const line = lineAt(value, caret);
  const marker = listMarker(line.text)?.marker;
  if (marker === undefined) return null;
  const width = indentWidth(marker.indent);
  let sibling: ListMarker | undefined;
  for (const above of markersAbove(value, line.start)) {
    const aboveWidth = indentWidth(above.marker.indent);
    if (aboveWidth < width) break;
    if (aboveWidth > width) {
      // A deeper line above is a child of the line to nest under: the nearest of the shallowest ones is the sibling.
      if (sibling === undefined || aboveWidth < indentWidth(sibling.indent)) sibling = above.marker;
      continue;
    }
    const indent = sibling?.indent ?? above.marker.indent + ' '.repeat(above.column - aboveWidth);
    const number = nestedNumber(marker, sibling);
    const replacedEnd = line.start + marker.indent.length + number.replaced;
    const text = indent + number.text;
    return edit(line.start, replacedEnd, text, caretAfter(caret, line.start, replacedEnd, { indent, text }));
  }
  return edit(line.start, line.start, '  ', caret + 2);
}

/**
 * Shift-Tab on an indented list line: it moves out one level and counts on from its new parent. With no parent, one
 * tab or up to two spaces go (03, The editor: List continuation rules).
 */
export function outdented(value: string, caret: number): Edit | null {
  const line = lineAt(value, caret);
  const marker = listMarker(line.text)?.marker;
  if (marker === undefined || marker.indent === '') return null;
  const parent = parentOf(value, line.start, marker);
  const indent = parent?.indent ?? marker.indent.replace(/^(?:\t| {1,2})/, '');
  // An ordered line under an ordered parent of the same kind counts on from it; any other line keeps its marker.
  const counted = parent?.kind === 'ordered' && marker.kind === 'ordered' && parent.delimiter === marker.delimiter ? parent : null;
  const number = counted === null ? { replaced: 0, text: '' } : nestedNumber(marker, counted);
  const replacedEnd = line.start + marker.indent.length + number.replaced;
  const text = indent + number.text;
  const caretThen = caretAfter(caret, line.start, replacedEnd, { indent, text });
  const below = counted === null ? null : renumbered(value, line.end, counted);
  if (below === null) return edit(line.start, replacedEnd, text, caretThen);
  return edit(line.start, below.end, text + value.slice(replacedEnd, line.end) + below.text, caretThen);
}
