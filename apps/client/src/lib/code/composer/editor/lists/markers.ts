/** The marker of a list line, as it is written (03, The editor: List continuation rules). */
export type ListMarker =
  | { readonly kind: 'ordered'; readonly indent: string; readonly numberText: string; readonly delimiter: '.' | ')'; readonly space: string }
  | { readonly kind: 'task'; readonly indent: string; readonly space: string }
  | { readonly kind: 'bullet'; readonly indent: string; readonly bullet: string; readonly space: string };

export interface ParsedMarker {
  readonly marker: ListMarker;
  /** Where the marker ends in the line. */
  readonly end: number;
  /** The column a nested line must reach to belong to this one. */
  readonly column: number;
}

/** How many columns the leading white space of `text` takes from `from`; a tab goes to the next stop of four. */
export function indentWidth(text: string, from = 0): number {
  let column = from;
  for (const char of text) {
    if (char === ' ') column += 1;
    else if (char === '\t') column += 4 - (column % 4);
    else break;
  }
  return column - from;
}

// Content follows one to four columns of padding; wider padding is code, so the column is one past the marker.
function contentColumn(indent: string, markerWidth: number, padding: string): number {
  const markerEnd = indentWidth(indent) + markerWidth;
  const space = indentWidth(padding, markerEnd);
  return markerEnd + (space >= 1 && space <= 4 ? space : 1);
}

function orderedMarker(indent: string, rest: string): ParsedMarker | null {
  const match = /^(\d+)([.)])((?:[ \t]+)|\s*$)/.exec(rest);
  if (match === null) return null;
  const [whole, numberText = '', delimiter, space = ''] = match;
  return {
    marker: { kind: 'ordered', indent, numberText, delimiter: delimiter === ')' ? ')' : '.', space },
    end: indent.length + whole.length,
    column: contentColumn(indent, numberText.length + 1, space),
  };
}

export function listMarker(line: string): ParsedMarker | null {
  const indent = /^[ \t]*/.exec(line)?.[0] ?? '';
  const rest = line.slice(indent.length);
  const ordered = orderedMarker(indent, rest);
  if (ordered !== null) return ordered;
  // The checkbox is part of the line's content, so a task's content starts after "- ".
  const task = /^-(\s)\[[ xX]\]((?:[ \t]+)|\s*$)/.exec(rest);
  if (task !== null) {
    return { marker: { kind: 'task', indent, space: task[2] ?? '' }, end: indent.length + task[0].length, column: contentColumn(indent, 1, task[1] ?? ' ') };
  }
  const bullet = /^([-*+])((?:[ \t]+)|\s*$)/.exec(rest);
  if (bullet === null) return null;
  const space = bullet[2] ?? '';
  return { marker: { kind: 'bullet', indent, bullet: bullet[1] ?? '-', space }, end: indent.length + bullet[0].length, column: contentColumn(indent, 1, space) };
}

/** The number after `numberText`, as wide as it is written (`09` gives `10`, `007` gives `008`). */
export function nextNumber(numberText: string): string {
  const number = Number.parseInt(numberText, 10);
  return Number.isSafeInteger(number) ? String(number + 1).padStart(numberText.length, '0') : numberText;
}

/** The marker of the line after `marker`: the same bullet, the next number, an unchecked task. */
export function nextMarker(marker: ListMarker): string {
  if (marker.kind === 'ordered') return `${marker.indent}${nextNumber(marker.numberText)}${marker.delimiter} `;
  if (marker.kind === 'task') return `${marker.indent}- [ ] `;
  return `${marker.indent}${marker.bullet} `;
}
