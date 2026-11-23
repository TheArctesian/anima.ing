/** `rows` holds both markers. */
export function replaceBlock(text: string, start: string, end: string, rows: string): string | undefined {
  const from = text.indexOf(start);
  const to = text.indexOf(end);
  if (from === -1 || to === -1) return undefined;
  return text.slice(0, from) + rows + text.slice(to + end.length);
}
