import type { Chip } from '../draft';
import type { Span } from './edit';

/**
 * The chips the editor can draw as one unit each: inside the prompt, not empty, on one line, in order and apart.
 * A chip that breaks one of these is left as text, so a wrong reader cannot break the editor.
 */
export function soundChips(prompt: string, chips: readonly Chip[]): Chip[] {
  const sound: Chip[] = [];
  let reached = 0;
  for (const chip of [...chips].sort((left, right) => left.start - right.start)) {
    const inside = Number.isInteger(chip.start) && Number.isInteger(chip.end) && chip.start >= reached && chip.end <= prompt.length;
    if (!inside || chip.end <= chip.start || prompt.slice(chip.start, chip.end).includes('\n')) continue;
    sound.push(chip);
    reached = chip.end;
  }
  return sound;
}

/** `span` made wider so it holds every chip it cuts: a chip goes, or is copied, whole (03, Removing and selecting chips). */
export function wholeChips(span: Span, chips: readonly Span[]): Span {
  let { start, end } = span;
  for (const chip of chips) {
    if (chip.start < start && chip.end > start) start = chip.start;
    if (chip.start < end && chip.end > end) end = chip.end;
  }
  return { start, end };
}

/** An offset moved out of a chip, to its nearer edge: the caret counts a chip as one position. */
export function outsideChips(at: number, chips: readonly Span[]): number {
  const chip = chips.find((one) => one.start < at && one.end > at);
  if (chip === undefined) return at;
  return at - chip.start < chip.end - at ? chip.start : chip.end;
}

/** Whether `span` touches a chip: its edge counts. */
export function touchesChip(span: Span, chips: readonly Span[]): boolean {
  return chips.some((chip) => chip.start < span.end && chip.end > span.start);
}
