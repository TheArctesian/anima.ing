import type { Chip } from '../../draft';
import type { Mode, Span } from '../edit';
import type { Line } from '../markdown/blocks';
import { runs, type Run, type Style } from '../markdown/inline';

/** One drawn stretch of a line: text, a Markdown marker, or a chip. `start` and `end` are offsets in the line. */
export interface Bit extends Span {
  readonly role: 'text' | 'marker' | 'chip';
  readonly text: string;
  readonly styles: readonly Style[];
  /** A marker the caret does not touch is in the page and not shown (03, The editor: How Markdown markers are shown and hidden). */
  readonly hidden: boolean;
  readonly chip: Chip | null;
  /** A chip that is the whole selection, or inside a wider one. */
  readonly picked: 'alone' | 'within' | null;
}

/** What to draw for one line. Two sketches with the same `key` draw the same thing. */
export interface Sketch {
  readonly line: Line;
  readonly bits: readonly Bit[];
  /** The caret is on a fence or a rule: its source shows. */
  readonly source: boolean;
  readonly key: string;
}

/** What the lines are drawn with: the mode, the chips of the prompt, and the selection while the editor has focus. */
export interface Look {
  readonly mode: Mode;
  readonly chips: readonly Chip[];
  readonly sel: Span | null;
}

// Stands for the characters of a chip while the line is read, so a chip's source is never read as Markdown.
const MASK = '';
const MEMO_SIZE = 4000;
const memo = new Map<string, Run[]>();

/** `runs` of a line, kept: the same lines are read again at every move of the caret. */
function runsOf(text: string): Run[] {
  const kept = memo.get(text);
  if (kept !== undefined) return kept;
  if (memo.size >= MEMO_SIZE) memo.clear();
  const made = runs(text);
  memo.set(text, made);
  return made;
}

function bit(role: Bit['role'], span: Span, text: string, more: Partial<Bit> = {}): Bit {
  return { role, start: span.start, end: span.end, text: text.slice(span.start, span.end), styles: [], hidden: false, chip: null, picked: null, ...more };
}

function touches(sel: Span | null, span: Span): boolean {
  return sel !== null && sel.start <= span.end && sel.end >= span.start;
}

function chipBit(chip: Span & { chip: Chip }, text: string, sel: Span | null): Bit {
  const covered = sel !== null && sel.start < sel.end && sel.start <= chip.start && sel.end >= chip.end;
  const alone = covered && sel.start === chip.start && sel.end === chip.end;
  return bit('chip', chip, text, { chip: chip.chip, picked: alone ? 'alone' : covered ? 'within' : null });
}

/** The bits of the text after the block marker: styled runs in `rich`, with their markers hidden until touched, and chips. */
function contentBits(line: Line, look: Look, sel: Span | null): Bit[] {
  const { text, prefix } = line;
  const end = line.start + text.length;
  const chips = look.chips
    .filter((chip) => chip.start >= line.start + prefix && chip.end <= end)
    .map((chip) => ({ start: chip.start - line.start, end: chip.end - line.start, chip }));
  const masked = chips.reduce((kept, chip) => kept.slice(0, chip.start - prefix) + MASK.repeat(chip.end - chip.start) + kept.slice(chip.end - prefix), text.slice(prefix));
  const found: readonly Run[] = look.mode === 'rich' ? runsOf(masked) : [{ start: 0, end: masked.length, styles: [], marker: false, region: null }];
  const bits: Bit[] = [];
  let next = 0;
  for (const run of found) {
    const span = { start: run.start + prefix, end: run.end + prefix };
    const region = run.region === null ? null : { start: run.region.start + prefix, end: run.region.end + prefix };
    const more = { styles: run.styles, hidden: run.marker && !(region !== null && touches(sel, region)) };
    let at = span.start;
    // A chip lies whole in one run: its masked characters are plain text to the reader.
    for (let chip = chips[next]; chip !== undefined && chip.start < span.end; chip = chips[next]) {
      if (chip.start > at) bits.push(bit(run.marker ? 'marker' : 'text', { start: at, end: chip.start }, text, more));
      bits.push(chipBit(chip, text, sel));
      at = chip.end;
      next += 1;
    }
    if (span.end > at) bits.push(bit(run.marker ? 'marker' : 'text', { start: at, end: span.end }, text, more));
  }
  return bits;
}

function lineBits(line: Line, look: Look, sel: Span | null, source: boolean): Bit[] {
  const { text, prefix, kind } = line;
  const whole = { start: 0, end: text.length };
  if (kind === 'code') return text === '' ? [] : [bit('text', whole, text)];
  if (kind === 'rule' || kind === 'close') return [bit('marker', whole, text, { hidden: !source })];
  const marker = prefix === 0 ? [] : [bit('marker', { start: 0, end: prefix }, text, { hidden: kind !== 'fence' || !source })];
  if (kind === 'fence') return prefix === text.length ? marker : [...marker, bit('text', { start: prefix, end: text.length }, text)];
  return [...marker, ...contentBits(line, look, sel)];
}

/** What to draw for `line` (03, The editor: How Markdown markers are shown and hidden). */
export function sketch(line: Line, look: Look): Sketch {
  const sel = look.sel === null ? null : { start: look.sel.start - line.start, end: look.sel.end - line.start };
  const fenced = line.kind === 'fence' || line.kind === 'close' || line.kind === 'rule';
  const source = fenced && touches(sel, { start: 0, end: line.text.length });
  const bits = lineBits(line, look, sel, source);
  const drawn = bits.map((one) => [one.role, one.start, one.end, one.hidden, one.picked, one.styles.join(' '), one.chip?.label, one.chip?.icon, one.chip?.detail]);
  const key = JSON.stringify([line.kind, line.level, line.depth, line.marker, line.gutter, line.checked, line.first, line.last, line.text, source, drawn]);
  return { line, bits, source, key };
}
