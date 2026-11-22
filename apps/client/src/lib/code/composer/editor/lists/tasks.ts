import { lineAt, type Edit } from '../edit';
import { checkAt } from '../markdown/blocks';

/**
 * The change that ticks or unticks the task on the line that holds `at`: `[ ]` becomes `[x]` and back. The selection
 * stays as it is, for the prompt keeps its length. Null when the line is no task.
 */
export function ticked(value: string, at: number, selection: { readonly anchor: number; readonly focus: number }): Edit | null {
  const line = lineAt(value, at);
  const check = checkAt(line.text);
  if (check === -1) return null;
  const start = line.start + check;
  return { start, end: start + 1, text: value[start] === ' ' ? 'x' : ' ', anchor: selection.anchor, focus: selection.focus };
}
