import { describe, expect, it } from 'vitest';
import { menuPlace, nextChoice, type MenuChoice } from './menu';
import { tipPlace } from './tip';

const choices: MenuChoice[] = [
  { kind: 'choice', id: 'a', label: 'Pin' },
  { kind: 'choice', id: 'b', label: 'Rename', disabled: true },
  { kind: 'choice', id: 'c', label: 'Archive' },
];

describe('keys in a menu', () => {
  it('moves past disabled choices and wraps at the ends', () => {
    expect(nextChoice('ArrowDown', 0, choices)).toBe(2);
    expect(nextChoice('ArrowDown', 2, choices)).toBe(0);
    expect(nextChoice('ArrowUp', 0, choices)).toBe(2);
    expect(nextChoice('Home', 2, choices)).toBe(0);
    expect(nextChoice('End', 0, choices)).toBe(2);
    expect(nextChoice('a', 0, choices)).toBeNull();
    expect(nextChoice('ArrowDown', 0, [{ kind: 'choice', id: 'x', label: 'X', disabled: true }])).toBeNull();
  });
});

describe('where a menu opens', () => {
  const viewport = { width: 800, height: 600 };
  const panel = { width: 160, height: 100 };

  it('opens above, lined up with the end edge, as the approval menu asks', () => {
    const anchor = { left: 300, top: 400, width: 40, height: 20 };
    expect(menuPlace(anchor, panel, viewport, { side: 'above', align: 'end', gap: 4 })).toEqual({ left: 180, top: 296 });
  });

  it('turns to the other side when the asked one has no room', () => {
    const anchor = { left: 10, top: 20, width: 40, height: 20 };
    expect(menuPlace(anchor, panel, viewport, { side: 'above', align: 'start', gap: 4 })).toEqual({ left: 10, top: 44 });
    const low = { left: 10, top: 560, width: 40, height: 20 };
    expect(menuPlace(low, panel, viewport, { side: 'below', align: 'start', gap: 4 })).toEqual({ left: 10, top: 456 });
  });

  it('centres a tip on its trigger, on the asked side when there is room, inside the window', () => {
    const tip = { width: 80, height: 24 };
    expect(tipPlace({ left: 100, top: 100, width: 40, height: 20 }, tip, viewport, { side: 'above', gap: 4 })).toEqual({ left: 80, top: 72 });
    expect(tipPlace({ left: 0, top: 10, width: 20, height: 20 }, tip, viewport, { side: 'above', gap: 4 })).toEqual({ left: 0, top: 34 });
    expect(tipPlace({ left: 100, top: 14, width: 28, height: 28 }, tip, viewport, { side: 'below', gap: 4 })).toEqual({ left: 74, top: 46 });
    expect(tipPlace({ left: 100, top: 570, width: 28, height: 28 }, tip, viewport, { side: 'below', gap: 4 })).toEqual({ left: 74, top: 542 });
  });
});
