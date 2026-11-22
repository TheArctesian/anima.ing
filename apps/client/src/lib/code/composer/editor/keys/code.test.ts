import { describe, expect, it } from 'vitest';
import { after, marked } from '../edit.fixture';
import { lines } from '../markdown/blocks';
import { codeBreak, codeIndent, codeUnwrap, fenceGuard } from './code';

function enter(text: string): string | null {
  const { value, caret } = marked(text);
  return after(value, codeBreak(value, caret, lines(value, 'rich')));
}

function tab(text: string, direction: 'in' | 'out'): string | null {
  const { value, sel } = marked(text);
  return after(value, codeIndent(value, sel, lines(value, 'rich'), direction));
}

function guard(text: string, back: boolean): string | null {
  const { value, caret } = marked(text);
  return after(value, fenceGuard(value, caret, lines(value, 'rich'), back));
}

describe('Enter in a code block', () => {
  it('gives an opening fence its block, closed, with the caret inside', () => {
    expect(enter('```ts|')).toBe('```ts\n|\n```');
    expect(enter('~~~~|\nrest')).toBe('~~~~\n|\n~~~~\nrest');
  });

  it('only breaks the line after a fence that has its block already', () => {
    expect(enter('```ts|\ncode\n```')).toBeNull();
    expect(enter('``|`ts')).toBeNull();
  });

  it('keeps the leading white space of the line', () => {
    expect(enter('```\n  if (x) {|\n```')).toBe('```\n  if (x) {\n  |\n```');
    expect(enter('```\n\tdeep|er\n```')).toBe('```\n\tdeep\n\t|er\n```');
    expect(enter('```\nflat|\n```')).toBeNull();
  });

  it('leaves the block after two blank lines at its end', () => {
    expect(enter('```\ncode\n\n|\n```')).toBe('```\ncode\n```\n|');
    expect(enter('```\ncode\n\n|\n```\nafter')).toBe('```\ncode\n```\n|\nafter');
  });

  it('closes an open block when it leaves it', () => {
    expect(enter('```\ncode\n\n|')).toBe('```\ncode\n```\n|');
  });

  it('has no rule outside a code block', () => {
    expect(enter('  text|')).toBeNull();
  });
});

describe('Tab in a code block', () => {
  it('puts two spaces at a bare caret', () => {
    expect(tab('```\na|b\n```', 'in')).toBe('```\na  |b\n```');
  });

  it('moves the selected lines in and out, and keeps them selected', () => {
    expect(tab('```\n[a\n\nb]\n```', 'in')).toBe('```\n  [a\n\n  b]\n```');
    expect(tab('```\n  [a\n\tb]\n c\n```', 'out')).toBe('```\n[a\nb]\n c\n```');
    expect(tab('```\n  a|\n```', 'out')).toBe('```\na|\n```');
  });

  it('leaves a line out that the selection only reaches the start of', () => {
    expect(tab('```\n[a\n]b\n```', 'in')).toBe('```\n  [a\n]b\n```');
  });

  it('does nothing outside the code of one block', () => {
    expect(tab('a|', 'in')).toBeNull();
    expect(tab('```|\na\n```', 'in')).toBeNull();
    expect(tab('[text\n```\na]\n```', 'in')).toBeNull();
  });
});

describe('the delete keys at the edges of a code block', () => {
  it('unwraps the block with Backspace at its start', () => {
    const { value, caret } = marked('before\n```ts\n|a\nb\n```\nafter');
    expect(after(value, codeUnwrap(value, caret, lines(value, 'rich')))).toBe('before\n|a\nb\nafter');
    const open = marked('```\n|a\nb');
    expect(after(open.value, codeUnwrap(open.value, open.caret, lines(open.value, 'rich')))).toBe('|a\nb');
    const inside = marked('```\na\n|b\n```');
    expect(codeUnwrap(inside.value, inside.caret, lines(inside.value, 'rich'))).toBeNull();
  });

  it('keeps the closing fence when Backspace is pressed under it', () => {
    expect(guard('```\na\n```\n|', true)).toBe('```\na\n```|');
    expect(guard('```\na\n```\n|text', true)).toBe('```\na\n```|\ntext');
    expect(guard('```\na\n```\nte|xt', true)).toBeNull();
  });

  it('keeps the closing fence when Delete is pressed above it', () => {
    expect(guard('```\na|\n```', false)).toBe('```\na|\n```');
    expect(guard('```\n|a\n```', false)).toBeNull();
  });
});
