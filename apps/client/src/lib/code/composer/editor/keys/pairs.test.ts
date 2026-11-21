import { describe, expect, it } from 'vitest';
import { after, marked } from '../edit.fixture';
import type { Style } from '../markdown/inline';
import { toggled, wrapped } from './pairs';

function wrap(text: string, open: string): string | null {
  const { value, sel } = marked(text);
  return after(value, wrapped(value, sel, open));
}

function toggle(text: string, style: Style): string | null {
  const { value, sel } = marked(text);
  return after(value, toggled(value, sel, style));
}

describe('typing an opening character over a selection', () => {
  it('wraps the selection and keeps it selected', () => {
    expect(wrap('a [word] b', '(')).toBe('a ([word]) b');
    expect(wrap('[x]', '“')).toBe('“[x]”');
    expect(wrap('[x]', '`')).toBe('`[x]`');
    expect(wrap('[x]', '*')).toBe('*[x]*');
    expect(wrap('[x]', '«')).toBe('«[x]»');
  });

  it('does nothing for another character, or with a bare caret', () => {
    expect(wrap('[x]', 'a')).toBeNull();
    expect(wrap('x|', '(')).toBeNull();
  });
});

describe('the style keys', () => {
  it('put the delimiters of the style around the selection', () => {
    expect(toggle('a [b] c', 'bold')).toBe('a **[b]** c');
    expect(toggle('a [b] c', 'italic')).toBe('a *[b]* c');
    expect(toggle('a [b] c', 'strike')).toBe('a ~~[b]~~ c');
    expect(toggle('a [b] c', 'code')).toBe('a `[b]` c');
  });

  it('take them away when they are right around the selection, or at its ends', () => {
    expect(toggle('a **[b]** c', 'bold')).toBe('a [b] c');
    expect(toggle('a [**b**] c', 'bold')).toBe('a [b] c');
    expect(toggle('a `[b]` c', 'code')).toBe('a [b] c');
    expect(toggle('~~[b]~~', 'strike')).toBe('[b]');
  });

  it('tell bold from italic in a run of asterisks', () => {
    expect(toggle('***[b]***', 'bold')).toBe('*[b]*');
    expect(toggle('***[b]***', 'italic')).toBe('**[b]**');
    expect(toggle('**[b]**', 'italic')).toBe('***[b]***');
    expect(toggle('*[b]*', 'bold')).toBe('***[b]***');
  });

  it('give a bare caret an empty pair to type in, and take it back', () => {
    expect(toggle('a |', 'bold')).toBe('a **|**');
    expect(toggle('a **|**', 'bold')).toBe('a |');
  });

  it('do nothing across lines', () => {
    expect(toggle('[a\nb]', 'bold')).toBeNull();
  });
});
