import { describe, expect, it } from 'vitest';
import { afterLeaving, afterToggle, foldsBar, shownFolded } from './code-fold';

describe('the bar folds while Code is open (OD-THR60)', () => {
  it('folds on the page Code only, not on its specimen', () => {
    expect(foldsBar('/code')).toBe(true);
    expect(foldsBar('/code/abc')).toBe(true);
    expect(foldsBar('/code-kit')).toBe(false);
    expect(foldsBar('/devices')).toBe(false);
  });

  it('shows the stored choice elsewhere, and folded on Code until the person opens it there', () => {
    const open = { stored: false, openedHere: false };
    expect(shownFolded(open, '/devices')).toBe(false);
    expect(shownFolded(open, '/code')).toBe(true);
    expect(shownFolded({ stored: false, openedHere: true }, '/code')).toBe(false);
    expect(shownFolded({ stored: true, openedHere: false }, '/devices')).toBe(true);
  });

  it('opening on Code is for the visit and is stored; folding on Code stores nothing', () => {
    const opened = afterToggle({ stored: true, openedHere: false }, false, '/code');
    expect(opened).toEqual({ fold: { stored: false, openedHere: true }, store: true });
    expect(afterToggle(opened.fold, true, '/code')).toEqual({ fold: { stored: false, openedHere: false }, store: false });
    expect(afterToggle({ stored: false, openedHere: false }, true, '/devices')).toEqual({ fold: { stored: true, openedHere: false }, store: true });
  });

  it('leaving Code ends the visit: coming back folds the bar again', () => {
    const left = afterLeaving({ stored: false, openedHere: true }, '/devices');
    expect(left).toEqual({ stored: false, openedHere: false });
    expect(shownFolded(left, '/code')).toBe(true);
  });
});
