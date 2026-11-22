/** The most bytes of a shown name, as the runner cuts it (`MOST_NAME_BYTES` of its `control/attach.rs`). */
const MOST_BYTES = 120;

/* The code points that do not show or change how text shows: marks of direction, spaces of no width, a soft hyphen, separators of lines, selectors of a variation, tags. */
const HIDDEN: readonly (readonly [number, number])[] = [
  [0xad, 0xad],
  [0x61c, 0x61c],
  [0x180e, 0x180e],
  [0x200b, 0x200f],
  [0x2028, 0x202e],
  [0x2060, 0x206f],
  [0xfe00, 0xfe0f],
  [0xfeff, 0xfeff],
  [0xfff9, 0xfffb],
  [0xe0000, 0xe007f],
  [0xe0100, 0xe01ef],
];
const CONTROL = /^\p{Cc}$/u;
const KEPT = /^(?:[._ -]|\p{Alphabetic}|\p{N})$/u;
/* The fillers of Hangul are letters that show nothing. */
const FILLERS: readonly number[] = [0x115f, 0x1160, 0x3164, 0xffa0];

function hidden(char: string): boolean {
  const point = char.codePointAt(0) ?? 0;
  return CONTROL.test(char) || HIDDEN.some(([from, to]) => point >= from && point <= to);
}

function utf8Bytes(char: string): number {
  const point = char.codePointAt(0) ?? 0;
  if (point < 0x80) return 1;
  if (point < 0x800) return 2;
  return point < 0x10000 ? 3 : 4;
}

/**
 * The name of a file as the runner shows it to the program (`shown_name`, TH111): the last part of the name, of
 * letters, digits and `._ -`; a different character that shows becomes `_`, one that does not is dropped. The runner's
 * answer does not carry this name, so the chip of a file makes it by the same rule: the person sees the name that the
 * program is told.
 */
export function shownName(name: string): string {
  const last = name.split(/[/\\]/).at(-1) ?? '';
  let safe = '';
  let bytes = 0;
  for (const char of last) {
    if (hidden(char)) continue;
    const kept = KEPT.test(char) && !FILLERS.includes(char.codePointAt(0) ?? 0) ? char : '_';
    if (kept === '_' && safe.endsWith('_')) continue;
    if (bytes + utf8Bytes(kept) > MOST_BYTES) break;
    safe += kept;
    bytes += utf8Bytes(kept);
  }
  const trimmed = safe.replace(/^[. -]+/, '').trimEnd();
  return trimmed === '' ? 'attachment' : trimmed;
}
