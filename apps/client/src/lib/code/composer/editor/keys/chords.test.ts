import { describe, expect, it } from 'vitest';
import type { Mode } from '../edit';
import { after, marked } from '../edit.fixture';
import { chordAction, leavesAfter, type Chord } from './chords';

const BARE = { shiftKey: false, altKey: false, ctrlKey: false, metaKey: false, mac: true };

/** Presses a key in a prompt and gives the prompt after it, a step of history, or null when the key is not ours. */
function press(text: string, chord: Partial<Chord> & { key: string }, mode: Mode = 'rich'): string | null {
  const { value, sel } = marked(text);
  const action = chordAction({ ...BARE, ...chord }, { value, sel, mode });
  return action === null || typeof action === 'string' ? action : after(value, action.edit);
}

describe('Enter, when the composer did not take it', () => {
  it('breaks the line', () => {
    expect(press('ab|cd', { key: 'Enter' })).toBe('ab\n|cd');
    expect(press('a[bc]d', { key: 'Enter', shiftKey: true })).toBe('a\n|d');
  });

  it('follows the rules of a list, a quote and a code block', () => {
    expect(press('- a|', { key: 'Enter' })).toBe('- a\n- |');
    expect(press('> a|', { key: 'Enter' })).toBe('> a\n> |');
    expect(press('> a\n> |', { key: 'Enter' })).toBe('> a\n|');
    expect(press('```js|', { key: 'Enter' })).toBe('```js\n|\n```');
  });

  it('reads no list inside a code block, and no rule as a list', () => {
    expect(press('```\n- a|\n```', { key: 'Enter' })).toBe('```\n- a\n|\n```');
    expect(press('- - -|', { key: 'Enter' })).toBe('- - -\n|');
  });

  it('continues a list in plain text, and has no rules in literal text', () => {
    expect(press('- a|', { key: 'Enter' }, 'plain')).toBe('- a\n- |');
    expect(press('- a|', { key: 'Enter' }, 'literal')).toBe('- a\n|');
  });

  it('groups a plain line break with typing, and a rule on its own', () => {
    const page = { value: '- a', sel: { start: 3, end: 3 }, mode: 'rich' as const };
    expect(chordAction({ ...BARE, key: 'Enter' }, page)).toMatchObject({ kind: 'other' });
    expect(chordAction({ ...BARE, key: 'Enter' }, { ...page, value: 'abc' })).toMatchObject({ kind: 'insert' });
  });
});

describe('Tab', () => {
  it('nests a list line, and Shift-Tab moves it out', () => {
    expect(press('- a\n- b|', { key: 'Tab' })).toBe('- a\n  - b|');
    expect(press('- a\n  - b|', { key: 'Tab', shiftKey: true })).toBe('- a\n- b|');
  });

  it('moves the lines of a code block', () => {
    expect(press('```\n[a\nb]\n```', { key: 'Tab' })).toBe('```\n  [a\n  b]\n```');
  });

  it('is left to move focus anywhere else', () => {
    expect(press('text|', { key: 'Tab' })).toBeNull();
    expect(press('- a\n- [b]', { key: 'Tab' })).toBeNull();
    expect(press('- a\n- b|', { key: 'Tab' }, 'literal')).toBeNull();
  });
});

describe('the Mod keys', () => {
  it('style the selection with Command on a Mac and Control elsewhere', () => {
    expect(press('[a]', { key: 'b', metaKey: true })).toBe('**[a]**');
    expect(press('[a]', { key: 'i', ctrlKey: true, mac: false })).toBe('*[a]*');
    expect(press('[a]', { key: 'S', metaKey: true, shiftKey: true })).toBe('~~[a]~~');
    expect(press('[a]', { key: 'e', metaKey: true })).toBe('`[a]`');
  });

  it('leave Control on a Mac to the system, where it moves the caret', () => {
    expect(press('[a]', { key: 'b', ctrlKey: true })).toBeNull();
    expect(press('[a]', { key: 'e', ctrlKey: true })).toBeNull();
  });

  it('style nothing in plain and literal text', () => {
    expect(press('[a]', { key: 'b', metaKey: true }, 'plain')).toBeNull();
  });

  it('undo and redo', () => {
    expect(press('a|', { key: 'z', metaKey: true })).toBe('undo');
    expect(press('a|', { key: 'Z', metaKey: true, shiftKey: true })).toBe('redo');
    expect(press('a|', { key: 'y', ctrlKey: true, mac: false })).toBe('redo');
    expect(press('a|', { key: 'y', metaKey: true })).toBeNull();
  });
});

describe('Escape, then Tab, out of the editor', () => {
  const at = (text: string, mode: Mode = 'rich') => ({ ...marked(text), mode });
  const CODE = at('```\na|\n```');

  it('arms the next Tab after Escape where Tab is the editor\'s: in a code block and on a list line', () => {
    expect(leavesAfter(false, 'Escape', CODE)).toBe(true);
    expect(leavesAfter(false, 'Escape', at('```\n[a\nb]\n```'))).toBe(true);
    expect(leavesAfter(false, 'Escape', at('- a|'))).toBe(true);
    expect(leavesAfter(false, 'Escape', at('- a\n  1. b|'))).toBe(true);
  });

  it('does not arm it where Tab moves focus already', () => {
    expect(leavesAfter(false, 'Escape', at('```\na\n```\nb|'))).toBe(false);
    expect(leavesAfter(false, 'Escape', at('# Title|'))).toBe(false);
    expect(leavesAfter(false, 'Escape', at('- a|', 'literal'))).toBe(false);
    expect(leavesAfter(false, 'Escape', at('```\na|\n```', 'literal'))).toBe(false);
  });

  it('keeps it while only a modifier goes down, so Shift+Tab leaves too', () => {
    expect(['Shift', 'Control', 'Alt', 'Meta'].map((key) => leavesAfter(true, key, CODE))).toEqual([true, true, true, true]);
    expect(leavesAfter(false, 'Shift', CODE)).toBe(false);
  });

  it('ends it at each other key, the Tab that leaves too', () => {
    expect(['Tab', 'a', 'ArrowDown', 'Enter', 'Backspace'].map((key) => leavesAfter(true, key, CODE))).toEqual([false, false, false, false, false]);
  });
});
