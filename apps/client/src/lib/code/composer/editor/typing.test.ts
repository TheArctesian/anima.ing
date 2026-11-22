import { describe, expect, it } from 'vitest';
import type { Mode, Span } from './edit';
import { after, marked } from './edit.fixture';
import { backspaced, LONG_PASTE_BYTES, pasteIsLong, PROMPT_LIMIT, typedAction, type Typed } from './typing';

interface Around {
  readonly mode?: Mode;
  readonly chips?: readonly Span[];
  readonly target?: Span;
}

/** Makes an input in a prompt and gives the prompt after it; null when nothing happens. */
function input(text: string, type: string, data: string | null = null, around: Around = {}): string | null {
  const { value, sel } = marked(text);
  const typed: Typed = { type, text: data, target: around.target ?? null };
  const action = typedAction(typed, { value, sel, mode: around.mode ?? 'rich', chips: around.chips ?? [] });
  return action === null || typeof action === 'string' ? action : after(value, action.edit);
}

describe('typing', () => {
  it('puts text at the caret and over a selection', () => {
    expect(input('a|c', 'insertText', 'b')).toBe('ab|c');
    expect(input('a[xyz]c', 'insertText', 'b')).toBe('ab|c');
  });

  it('replaces the stretch a correction names, not the selection', () => {
    expect(input('teh |', 'insertReplacementText', 'the', { target: { start: 0, end: 3 } })).toBe('the| ');
  });

  it('wraps a selection in a pair, but not in literal text, and not over a chip', () => {
    expect(input('a[b]c', 'insertText', '(')).toBe('a([b])c');
    expect(input('a[b]c', 'insertText', '(', { mode: 'literal' })).toBe('a(|c');
    expect(input('a[b]c', 'insertText', '(', { chips: [{ start: 1, end: 2 }] })).toBe('a(|c');
  });

  it('breaks the line by the rules of Enter', () => {
    expect(input('- a|', 'insertParagraph')).toBe('- a\n- |');
    expect(input('a|b', 'insertLineBreak')).toBe('a\n|b');
  });

  it('groups typing as insert, deleting as delete, and a rewrite as other', () => {
    const desk = { value: 'ab', sel: { start: 2, end: 2 }, mode: 'rich' as const, chips: [] };
    expect(typedAction({ type: 'insertText', text: 'c', target: null }, desk)).toMatchObject({ kind: 'insert' });
    expect(typedAction({ type: 'deleteContentBackward', text: null, target: null }, desk)).toMatchObject({ kind: 'delete' });
    expect(typedAction({ type: 'insertText', text: 'c', target: null }, { ...desk, sel: { start: 0, end: 2 } })).toMatchObject({ kind: 'other' });
    expect(typedAction({ type: 'historyUndo', text: null, target: null }, desk)).toBe('undo');
    expect(typedAction({ type: 'historyRedo', text: null, target: null }, desk)).toBe('redo');
  });
});

describe('deleting', () => {
  it('takes a character, a word or a selection', () => {
    expect(input('ab|c', 'deleteContentBackward')).toBe('a|c');
    expect(input('ab|c', 'deleteContentForward')).toBe('ab|');
    expect(input('one two|', 'deleteWordBackward')).toBe('one |');
    expect(input('a[bc]d', 'deleteContentBackward')).toBe('a|d');
    expect(input('|abc', 'deleteContentBackward')).toBeNull();
  });

  it('takes a chip whole', () => {
    const chips = [{ start: 2, end: 10 }];
    expect(input('a @file.ts| b', 'deleteContentBackward', null, { chips })).toBe('a | b');
    expect(input('a |@file.ts b', 'deleteContentForward', null, { chips })).toBe('a | b');
    expect(input('[a @fi]le.ts b', 'deleteContentBackward', null, { chips })).toBe('| b');
  });

  it('takes the marker of a block before anything else', () => {
    expect(input('a\n# |b', 'deleteContentBackward')).toBe('a\n|b');
    expect(input('```\n|a\n```', 'deleteContentBackward')).toBe('|a');
    expect(input('```\na\n```\n|', 'deleteContentBackward')).toBe('```\na\n```|');
    expect(input('# |b', 'deleteContentBackward', null, { mode: 'plain' })).toBe('#|b');
  });

  it('keeps a soft-line delete of the browser inside the line', () => {
    expect(input('ab\ncd ef|', 'deleteSoftLineBackward', null, { target: { start: 1, end: 8 } })).toBe('ab\n|');
    expect(input('ab\ncd ef|', 'deleteSoftLineBackward', null, { target: { start: 6, end: 8 } })).toBe('ab\ncd |');
  });
});

describe('the format inputs of a menu', () => {
  it('style the selection in rich text only', () => {
    expect(input('[a]', 'formatBold')).toBe('**[a]**');
    expect(input('[a]', 'formatStrikeThrough')).toBe('~~[a]~~');
    expect(input('[a]', 'formatBold', null, { mode: 'plain' })).toBeNull();
    expect(input('[a]', 'formatUnderline')).toBeNull();
  });
});

describe('a long paste', () => {
  it('is 32 KB or more, counted in bytes', () => {
    expect(pasteIsLong('a'.repeat(LONG_PASTE_BYTES - 1), 0)).toBe(false);
    expect(pasteIsLong('a'.repeat(LONG_PASTE_BYTES), 0)).toBe(true);
    expect(pasteIsLong('é'.repeat(LONG_PASTE_BYTES / 2), 0)).toBe(true);
  });

  it('is any text that takes the prompt over its limit', () => {
    expect(pasteIsLong('abc', PROMPT_LIMIT - 3)).toBe(false);
    expect(pasteIsLong('abc', PROMPT_LIMIT - 2)).toBe(true);
  });
});

describe('Backspace as a key, for the engine that sends no input at the start of the first line', () => {
  const HELD = { shiftKey: false, altKey: false, ctrlKey: false, metaKey: false, mac: true };
  const key = (text: string, held: Partial<typeof HELD> = {}, mode: Mode = 'rich'): string | null => {
    const { value, sel } = marked(text);
    const action = backspaced({ ...HELD, ...held, key: 'Backspace' }, { value, sel, mode, chips: [] });
    return action === null || typeof action === 'string' ? action : after(value, action.edit);
  };

  it('takes the marker of a list line, a heading and a quote, and the fences of a code block', () => {
    expect(key('- |task')).toBe('|task');
    expect(key('- |')).toBe('|');
    expect(key('# |Title')).toBe('|Title');
    expect(key('> |said')).toBe('|said');
    expect(key('```\n|code\n```')).toBe('|code');
  });

  it('leaves each other Backspace to the input that follows it', () => {
    expect(key('- ta|sk')).toBeNull();
    expect(key('|plain')).toBeNull();
    expect(key('- [ta]sk')).toBeNull();
    expect(key('- |task', { altKey: true })).toBeNull();
    expect(key('- |task', { metaKey: true })).toBeNull();
    expect(key('- |task', {}, 'literal')).toBeNull();
  });
});
