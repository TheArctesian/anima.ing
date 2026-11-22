import { describe, expect, it } from 'vitest';
import type { Chip } from '../../draft';
import type { Mode, Span } from '../edit';
import { lines } from '../markdown/blocks';
import { sketch, type Sketch } from './sketch';

function sketches(markdown: string, sel: Span | null, more: { mode?: Mode; chips?: readonly Chip[] } = {}): Sketch[] {
  const mode = more.mode ?? 'rich';
  return lines(markdown, mode).map((line) => sketch(line, { mode, chips: more.chips ?? [], sel }));
}

/** A line as it is seen: hidden markers left out, shown markers in angle brackets, chips in braces. */
function seen(drawn: Sketch): string {
  return drawn.bits
    .filter((bit) => !bit.hidden)
    .map((bit) => (bit.role === 'marker' ? `<${bit.text}>` : bit.role === 'chip' ? `{${bit.chip?.label ?? ''}}` : bit.text))
    .join('');
}

function caret(at: number): Span {
  return { start: at, end: at };
}

describe('how Markdown markers are shown and hidden', () => {
  it('hides the markers of styled text the caret does not touch', () => {
    expect(sketches('a **b** `c`', caret(0)).map(seen)).toEqual(['a b c']);
    expect(sketches('a **b** `c`', null).map(seen)).toEqual(['a b c']);
  });

  it('shows the markers of the styled text the caret or the selection touches, and only those', () => {
    expect(sketches('a **b** `c`', caret(2)).map(seen)).toEqual(['a <**>b<**> c']);
    expect(sketches('a **b** `c`', caret(7)).map(seen)).toEqual(['a <**>b<**> c']);
    expect(sketches('a **b** `c`', caret(9)).map(seen)).toEqual(['a b <`>c<`>']);
    expect(sketches('a **b** `c`', { start: 0, end: 11 }).map(seen)).toEqual(['a <**>b<**> <`>c<`>']);
  });

  it('never shows the marker of a heading, a list line or a quote', () => {
    expect(sketches('# T\n- a\n> q', { start: 0, end: 11 }).map(seen)).toEqual(['T', 'a', 'q']);
  });

  it('shows a rule and a fence as their source while the caret is on them', () => {
    expect(sketches('---\n```ts\nx\n```', caret(0)).map(seen)).toEqual(['<--->', 'ts', 'x', '']);
    expect(sketches('---\n```ts\nx\n```', caret(6)).map(seen)).toEqual(['', '<```>ts', 'x', '']);
    expect(sketches('---\n```ts\nx\n```', caret(15)).map(seen)).toEqual(['', 'ts', 'x', '<```>']);
  });

  it('keeps every character of the line in its bits', () => {
    for (const drawn of sketches('## a **b** *c*\n- [x] `d` ~~e~~\n```\nf\n```', caret(3))) {
      expect(drawn.bits.map((bit) => bit.text).join('')).toBe(drawn.line.text);
    }
  });

  it('draws only text in plain and literal mode', () => {
    expect(sketches('# a **b**', caret(0), { mode: 'plain' }).map(seen)).toEqual(['# a **b**']);
    expect(sketches('# a **b**', caret(0), { mode: 'literal' }).map(seen)).toEqual(['# a **b**']);
  });
});

describe('chips in a line', () => {
  const prompt = 'see [a_b](src/a_b.ts) **now**';
  const chips: Chip[] = [{ start: 4, end: 21, label: 'a_b.ts', icon: 'file', detail: 'src/a_b.ts' }];

  it('draws a chip as one unit, and reads none of its source as Markdown', () => {
    expect(sketches(prompt, null, { chips }).map(seen)).toEqual(['see {a_b.ts} now']);
    expect(sketches(`*${prompt}*`, caret(0), { chips: [{ ...chips[0] as Chip, start: 5, end: 22 }] }).map(seen)).toEqual(['<*>see {a_b.ts} <**>now<***>']);
  });

  it('draws chips in plain text too', () => {
    expect(sketches(prompt, null, { chips, mode: 'plain' }).map(seen)).toEqual(['see {a_b.ts} **now**']);
  });

  it('marks a chip that is the whole selection, or inside a wider one', () => {
    const picked = (sel: Span | null) => sketches(prompt, sel, { chips })[0]?.bits.find((bit) => bit.role === 'chip')?.picked;
    expect(picked({ start: 4, end: 21 })).toBe('alone');
    expect(picked({ start: 0, end: 25 })).toBe('within');
    expect(picked(caret(4))).toBeNull();
    expect(picked({ start: 0, end: 10 })).toBeNull();
  });
});

describe('what makes a line be drawn again', () => {
  it('is a change of what is seen, not a move of the caret in plain text', () => {
    const key = (sel: Span | null) => sketches('plain **b**', sel)[0]?.key;
    expect(key(caret(0))).toBe(key(caret(3)));
    expect(key(caret(0))).not.toBe(key(caret(8)));
    expect(sketches('plain', null)[0]?.key).not.toBe(sketches('plain!', null)[0]?.key);
  });
});
