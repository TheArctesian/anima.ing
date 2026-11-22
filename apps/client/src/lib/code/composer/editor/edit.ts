/** A stretch of the prompt: offsets in the Markdown string, the end not included. */
export interface Span {
  readonly start: number;
  readonly end: number;
}

/** What the editor draws: all of Markdown, only paragraphs and chips, or the bare text (03, The editor: What it is). */
export type Mode = 'rich' | 'plain' | 'literal';

/** One change of the prompt: the stretch that goes, the text that comes, and the selection after it. */
export interface Edit extends Span {
  readonly text: string;
  readonly anchor: number;
  readonly focus: number;
}

/** An edit that leaves the caret at `caret`. */
export function edit(start: number, end: number, text: string, caret = start + text.length): Edit {
  return { start, end, text, anchor: caret, focus: caret };
}

export function applied(value: string, change: Edit): string {
  return value.slice(0, change.start) + change.text + value.slice(change.end);
}

/** The line of `value` that holds `at`: where it starts and ends (before its line break) and its text. */
export function lineAt(value: string, at: number): Span & { readonly text: string } {
  const start = value.lastIndexOf('\n', at - 1) + 1;
  const stop = value.indexOf('\n', at);
  const end = stop === -1 ? value.length : stop;
  return { start, end, text: value.slice(start, end) };
}
