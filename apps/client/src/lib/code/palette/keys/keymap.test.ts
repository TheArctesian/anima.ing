import { describe, expect, it } from 'vitest';
import { chordText, KEY_RULES, parseChord, pressedIs } from './keymap';
import { keyTable } from './table';
import { COMMAND_WORDS } from './words';

const press = (key: string, mods: Partial<Record<'metaKey' | 'ctrlKey' | 'shiftKey' | 'altKey', boolean>> = {}) => ({
  key,
  metaKey: false,
  ctrlKey: false,
  shiftKey: false,
  altKey: false,
  ...mods,
});

describe('chords of the key table', () => {
  it('reads T3 Code key syntax, the plus key among it', () => {
    expect(parseChord('mod+shift+k')).toEqual({ key: 'k', mod: true, meta: false, ctrl: false, shift: true, alt: false });
    expect(parseChord('mod++')?.key).toBe('+');
    expect(parseChord('esc')?.key).toBe('escape');
    expect(parseChord('mod+k+j')).toBeNull();
    expect(parseChord('mod+')).toBeNull();
  });

  it('writes a chord with the marks of a Mac, and with words elsewhere', () => {
    const chord = parseChord('mod+alt+shift+arrowup');
    if (chord === null) throw new Error('no chord');
    expect(chordText(chord, true)).toBe('⌥⇧⌘↑');
    expect(chordText(chord, false)).toBe('Ctrl+Alt+Shift+↑');
    expect(chordText(parseChord('shift+f6') ?? chord, true)).toBe('⇧F6');
  });

  it('takes Command for mod on a Mac and Ctrl elsewhere, and no more modifiers than the chord', () => {
    const chord = parseChord('mod+k');
    if (chord === null) throw new Error('no chord');
    expect(pressedIs(press('k', { metaKey: true }), chord, true)).toBe(true);
    expect(pressedIs(press('K', { ctrlKey: true }), chord, false)).toBe(true);
    expect(pressedIs(press('k', { ctrlKey: true }), chord, true)).toBe(false);
    expect(pressedIs(press('k', { metaKey: true, shiftKey: true }), chord, true)).toBe(false);
  });

  it('matches a shifted symbol and another layout by the place of the key, and the plus key with or without Shift', () => {
    const at = (key: string) => parseChord(key) ?? { key: '', mod: false, meta: false, ctrl: false, shift: false, alt: false };
    expect(pressedIs({ ...press('{', { metaKey: true, shiftKey: true }), code: 'BracketLeft' }, at('mod+shift+['), true)).toBe(true);
    expect(pressedIs({ ...press('!', { metaKey: true, shiftKey: true }), code: 'Digit1' }, at('mod+shift+1'), true)).toBe(true);
    expect(pressedIs({ ...press('л', { ctrlKey: true }), code: 'KeyK' }, at('mod+k'), false)).toBe(true);
    // ⌃⇧ on the Mac keeps the letter of the layout: Z on a French keyboard is Z, at the place of W.
    expect(pressedIs({ ...press('Z', { ctrlKey: true, shiftKey: true }), code: 'KeyW' }, at('ctrl+shift+z'), true)).toBe(true);
    expect(pressedIs({ ...press('Z', { ctrlKey: true, shiftKey: true }), code: 'KeyW' }, at('ctrl+shift+w'), true)).toBe(false);
    // A French layout types A at the place of Q: the letter typed wins.
    expect(pressedIs({ ...press('a', { ctrlKey: true }), code: 'KeyQ' }, at('mod+q'), false)).toBe(false);
    expect(pressedIs({ ...press('+', { ctrlKey: true, shiftKey: true }), code: 'Equal' }, at('mod++'), false)).toBe(true);
    expect(pressedIs({ ...press('+', { ctrlKey: true }), code: 'NumpadAdd' }, at('mod++'), false)).toBe(true);
  });

  it('has words for every command of the table, and every rule parses', () => {
    for (const rule of KEY_RULES) {
      expect(parseChord(rule.key), rule.key).not.toBeNull();
      expect(COMMAND_WORDS[rule.command], rule.command).toBeDefined();
    }
  });
});

describe('the key table', () => {
  it('folds the keys of one command into one line and the nine jumps into one', () => {
    const lines = keyTable(true).flatMap((group) => group.lines);
    expect(lines.find((line) => line.label === 'New chat')?.keys).toEqual(['⌘N', '⇧⌘O']);
    expect(lines.find((line) => line.label === 'Chat 1 to 9 in the list')?.keys).toEqual(['⌘1 – ⌘9']);
    expect(lines.filter((line) => line.label.startsWith('Chat 1 '))).toHaveLength(1);
  });

  it('keeps one command apart where its condition differs', () => {
    const close = keyTable(false)
      .flatMap((group) => group.lines)
      .filter((line) => line.keys.includes('Ctrl+W'));
    expect(close.map((line) => [line.label, line.where])).toEqual([
      ['Close the terminal', 'In a terminal'],
      ['Close the tab of the right panel', 'Not in a terminal'],
    ]);
  });

  it('lists the groups in order, with the palette key first', () => {
    const groups = keyTable(true);
    expect(groups.map((group) => group.label)).toEqual(['General', 'Chats', 'Composer', 'Terminal', 'Panels', 'Browser', 'Agent usage']);
    expect(groups[0]?.lines[0]).toEqual({ command: 'commandPalette.toggle', label: 'Open or close the command palette', keys: ['⌘K'], where: 'Not in a terminal' });
  });
});
