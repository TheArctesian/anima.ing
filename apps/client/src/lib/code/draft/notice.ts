/**
 * The lines of a folder's notice (THR-046) as the draft shows them. A line that ends with ":" heads the lines after it,
 * which are a list; a line before any heading is a paragraph of its own, such as the runner's warning about a folder
 * that holds Anima's own folder. Pure.
 */
export interface NoticePart {
  readonly words: string;
  readonly items: readonly string[];
}

export function noticeParts(lines: readonly string[]): readonly NoticePart[] {
  const parts: { words: string; items: string[] }[] = [];
  for (const line of lines) {
    const last = parts.at(-1);
    if (last !== undefined && last.words.endsWith(':') && !line.endsWith(':')) last.items.push(line);
    else parts.push({ words: line, items: [] });
  }
  return parts;
}
