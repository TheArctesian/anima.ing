import { describe, expect, it } from 'vitest';
import {
  NO_TABS, SURFACES, closed, closedOthers, closedToTheRight, iconOf, moved, opened, shown, surfaceForKey,
  type PanelTab, type PanelTabs,
} from './tabs';
import { DEFAULT_WIDTH, MIN_WIDTH, clamped, keepWidth, maxWidth, storedWidth } from './width';

const tab = (id: string, kind: PanelTab['kind'] = 'files'): PanelTab => ({ id, kind, title: id });
const three: PanelTabs = [tab('a'), tab('b'), tab('c')].reduce(opened, NO_TABS);
const ids = (state: PanelTabs): string[] => state.tabs.map((each) => each.id);

describe('the tabs of the right panel (TC5)', () => {
  it('TC5: the surfaces have the words and the keys of T3 Code, each key once', () => {
    expect(SURFACES.map((surface) => `${surface.label}:${surface.key}`)).toEqual([
      'Browser:b', 'Terminal:t', 'Files:f', 'Diff:d', 'Pull request:p', 'Linked items:l', 'Device:m', 'Side chat:s',
    ]);
    expect(surfaceForKey('D')?.kind).toBe('diff');
    expect(surfaceForKey('x')).toBeUndefined();
    expect(iconOf({ id: 'f', kind: 'file', title: 'a.ts', path: 'a.ts' })).toBe('carbon:document');
  });

  it('TC5: a tab that is open is shown again and not opened twice', () => {
    const again = opened(three, tab('a'));
    expect(ids(again)).toEqual(['a', 'b', 'c']);
    expect(again.active).toBe('a');
    expect(shown(three, 'nothing')).toBe(three);
  });

  it('TC5: after a close the tab at the right shows, else the one at the left, else none', () => {
    expect(closed(shown(three, 'b'), 'b').active).toBe('c');
    expect(closed(three, 'c').active).toBe('b');
    expect(closed(shown(three, 'a'), 'c').active).toBe('a');
    expect(closed(opened(NO_TABS, tab('a')), 'a')).toEqual(NO_TABS);
  });

  it('TC5: close others, close to the right and a move keep a tab that shows', () => {
    expect(closedOthers(three, 'b')).toEqual({ tabs: [tab('b')], active: 'b' });
    const right = closedToTheRight(three, 'a');
    expect(ids(right)).toEqual(['a']);
    expect(right.active).toBe('a');
    expect(closedToTheRight(shown(three, 'a'), 'b').active).toBe('a');
    expect(ids(moved(three, 'c', 0))).toEqual(['c', 'a', 'b']);
    expect(ids(moved(three, 'a', 99))).toEqual(['b', 'c', 'a']);
    expect(moved(three, 'a', 1).active).toBe('c');
  });
});

describe('the width of the right panel (TC5)', () => {
  it('TC5: the panel is never under 360px, and the chat column keeps 360px', () => {
    expect(maxWidth(1440, 1100)).toBe(740);
    expect(clamped(2000, 1440, 1100)).toBe(740);
    expect(clamped(100, 1440, 1100)).toBe(MIN_WIDTH);
    expect(clamped(900, 1000, 1000)).toBe(640);
    expect(clamped(Number.NaN, 1440, 1100)).toBe(DEFAULT_WIDTH);
  });

  it('TC5: the width is kept for each thread, and a store that fails gives the first width', () => {
    const kept = new Map<string, string>();
    const store = { getItem: (key: string) => kept.get(key) ?? null, setItem: (key: string, value: string) => void kept.set(key, value) };
    keepWidth(store, 'one', 612.4);
    expect(storedWidth(store, 'one')).toBe(612);
    expect(storedWidth(store, 'two')).toBe(DEFAULT_WIDTH);
    kept.set('anima.code.panel-width:two', 'wide');
    expect(storedWidth(store, 'two')).toBe(DEFAULT_WIDTH);
    const broken = { getItem: () => { throw new Error('no store'); }, setItem: () => { throw new Error('no store'); } };
    expect(storedWidth(broken, 'one')).toBe(DEFAULT_WIDTH);
    expect(() => keepWidth(broken, 'one', 500)).not.toThrow();
  });
});
