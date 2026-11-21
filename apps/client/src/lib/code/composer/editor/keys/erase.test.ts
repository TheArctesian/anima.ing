import { describe, expect, it } from 'vitest';
import { after, marked } from '../edit.fixture';
import { lines } from '../markdown/blocks';
import { erased, inLine, unprefixed, type Reach } from './erase';

function taken(text: string, back: boolean, reach: Reach): string {
  const { value, caret } = marked(text);
  const span = erased(value, caret, back, reach);
  return value.slice(span.start, span.end);
}

describe('what a delete key takes from a bare caret', () => {
  it('takes one character as a reader sees it', () => {
    expect(taken('ab|c', true, 'char')).toBe('b');
    expect(taken('ab|c', false, 'char')).toBe('c');
    expect(taken('a👍🏽|', true, 'char')).toBe('👍🏽');
    expect(taken('|é', false, 'char')).toBe('é');
  });

  it('takes a word with the space after it, or a run of signs', () => {
    expect(taken('one two |three', true, 'word')).toBe('two ');
    expect(taken('one| two three', false, 'word')).toBe(' two');
    expect(taken('a ==|', true, 'word')).toBe('==');
  });

  it('takes the rest of the line', () => {
    expect(taken('a\nbc|d\ne', true, 'line')).toBe('bc');
    expect(taken('a\nbc|d\ne', false, 'line')).toBe('d');
  });

  it('takes the line break at the edge of a line, and nothing at the edge of the prompt', () => {
    expect(taken('a\n|b', true, 'word')).toBe('\n');
    expect(taken('a|\nb', false, 'line')).toBe('\n');
    expect(taken('|a', true, 'char')).toBe('');
    expect(taken('a|', false, 'char')).toBe('');
  });

  it('keeps what the browser would take inside the line of the caret', () => {
    expect(inLine('ab\ncd ef', 6, { start: 1, end: 6 }, true)).toEqual({ start: 3, end: 6 });
    expect(inLine('ab cd\nef', 2, { start: 2, end: 8 }, false)).toEqual({ start: 2, end: 5 });
    expect(inLine('ab\ncd', 3, { start: 0, end: 3 }, true)).toEqual({ start: 2, end: 3 });
  });
});

describe('Backspace where the text of a block starts', () => {
  function backspace(text: string): string | null {
    const { value, caret } = marked(text);
    return after(value, unprefixed(caret, lines(value, 'rich')));
  }

  it('takes the marker away and leaves the text', () => {
    expect(backspace('## |Title')).toBe('|Title');
    expect(backspace('a\n  - |b')).toBe('a\n|b');
    expect(backspace('- [x] |done')).toBe('|done');
    expect(backspace('> |quote')).toBe('|quote');
    expect(backspace('12. |')).toBe('|');
  });

  it('is a plain Backspace anywhere else', () => {
    expect(backspace('## T|itle')).toBeNull();
    expect(backspace('|text')).toBeNull();
    expect(backspace('```|')).toBeNull();
    expect(backspace('---|')).toBeNull();
  });
});
