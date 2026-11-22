/** The lines of a text, as the source view numbers them: a last line break gives a last empty line. */
export function sourceLines(text: string): readonly string[] {
  return text.split(/\r\n|\n|\r/);
}

/** A line number held to the lines of the file: 1 at least, the last line at most. */
export function clampLine(line: number, count: number): number {
  if (!Number.isFinite(line)) return 1;
  return Math.max(1, Math.min(Math.max(1, count), Math.floor(line)));
}

/** Where a scroller goes so a line is in its centre. */
export function centredTop(line: { top: number; height: number }, viewport: number): number {
  return Math.max(0, line.top - (viewport - line.height) / 2);
}

const FRONT_MATTER = /^---[ \t]*\r?\n[\s\S]*?\r?\n(?:---|\.\.\.)[ \t]*(?:\r?\n|$)/;

/** A Markdown text with no front matter at its top (3.11). */
export function withoutFrontMatter(text: string): string {
  return text.replace(FRONT_MATTER, '');
}
