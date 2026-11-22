import { describe, expect, it } from 'vitest';
import { after, marked } from '../edit.fixture';
import { continued, indented, outdented } from './continuation';
import { ticked } from './tasks';

function enter(text: string): string | null {
  const { value, caret } = marked(text);
  return after(value, continued(value, caret));
}

function tab(text: string): string | null {
  const { value, caret } = marked(text);
  return after(value, indented(value, caret));
}

function shiftTab(text: string): string | null {
  const { value, caret } = marked(text);
  return after(value, outdented(value, caret));
}

describe('Enter in a list', () => {
  it('starts a line with the same bullet, the next number, an unchecked task', () => {
    expect(enter('- a|')).toBe('- a\n- |');
    expect(enter('* a|')).toBe('* a\n* |');
    expect(enter('3) a|')).toBe('3) a\n4) |');
    expect(enter('09. a|')).toBe('09. a\n10. |');
    expect(enter('- [x] a|')).toBe('- [x] a\n- [ ] |');
    expect(enter('  - a|')).toBe('  - a\n  - |');
  });

  it('splits the line at the caret', () => {
    expect(enter('- ab|cd')).toBe('- ab\n- |cd');
  });

  it('counts the ordered lines below on', () => {
    expect(enter('1. a|\n2. b\n3. c')).toBe('1. a\n2. |\n3. b\n4. c');
    expect(enter('1. a|\n2. b\n\n3. c')).toBe('1. a\n2. |\n3. b\n\n3. c');
    expect(enter('1. a|\n   - deep\n2. b')).toBe('1. a\n2. |\n   - deep\n3. b');
  });

  it('takes the marker of an empty line away', () => {
    expect(enter('- a\n- |')).toBe('- a\n|');
    expect(enter('1. |')).toBe('|');
  });

  it('moves an empty nested line to the list above it', () => {
    expect(enter('- a\n  - |')).toBe('- a\n- |');
    expect(enter('1. a\n   - |\n2. b')).toBe('1. a\n2. |\n3. b');
  });

  it('does nothing outside the text of a list line', () => {
    expect(enter('plain|')).toBeNull();
    expect(enter('|- a')).toBeNull();
    expect(enter('-|')).toBeNull();
  });
});

describe('Tab and Shift-Tab in a list', () => {
  it('nests a line under the one above, at the column of its text', () => {
    expect(tab('- a\n- b|')).toBe('- a\n  - b|');
    expect(tab('1. a\n2. b|')).toBe('1. a\n   1. b|');
    expect(tab('10. a\n11. |b')).toBe('10. a\n    1. |b');
  });

  it('counts on from a line already nested there', () => {
    expect(tab('1. a\n   1. x\n2. b|')).toBe('1. a\n   1. x\n   2. b|');
  });

  it('adds two spaces when there is no line to nest under', () => {
    expect(tab('- a|')).toBe('  - a|');
    expect(tab('not a list|')).toBeNull();
  });

  it('moves a nested line out one level, and it counts on from its parent', () => {
    expect(shiftTab('- a\n  - b|')).toBe('- a\n- b|');
    expect(shiftTab('1. a\n   1. b|\n2. c')).toBe('1. a\n2. b|\n3. c');
    expect(shiftTab('  - a|')).toBe('- a|');
    expect(shiftTab('- a|')).toBeNull();
  });
});

describe('the box of a task', () => {
  it('is ticked and unticked in the Markdown, and the selection stays', () => {
    const selection = { anchor: 7, focus: 7 };
    expect(after('- [ ] a', ticked('- [ ] a', 0, selection))).toBe('- [x] a|');
    expect(after('x\n- [X] a', ticked('x\n- [X] a', 4, selection))).toBe('x\n- [ ]| a');
    expect(ticked('- a', 0, selection)).toBeNull();
  });
});
