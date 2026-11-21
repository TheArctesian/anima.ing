import { describe, expect, it } from 'vitest';
import { clashes, TAKEN_KEYS, type Clash } from './clashes';
import { KEY_RULES } from './keymap';

const text = (found: readonly Clash[]) => found.map((clash) => `${clash.client} ${clash.platform} ${clash.key} ${clash.command}`);

describe('the keys of the Code section against the keys Anima, its window and the browser already take', () => {
  it('leaves only the clashes the ruling keeps: ⌘W and ⌘Z of the Mac menu in the desktop app', () => {
    const found = clashes();
    expect(text(found)).toEqual(['desktop mac mod+w terminal.close', 'desktop mac mod+w rightPanel.close', 'desktop mac mod+z thread.undo']);
    expect(found.every((clash) => clash.taken.kept !== undefined)).toBe(true);
  });

  it('would meet the browser on T3 Code keys, which is why the web client has its own', () => {
    const t3InBrowser = clashes(TAKEN_KEYS, (set) => (set.web ? KEY_RULES.filter((rule) => rule.when?.includes('isDesktop') !== true) : []));
    expect(text(t3InBrowser)).toEqual(expect.arrayContaining(['web mac mod+n chat.new', 'web other mod+k commandPalette.toggle', 'web mac mod+shift+[ thread.previous', 'web other mod+shift+c thread.copyReference']));
    expect(text(t3InBrowser)).not.toContain('web mac mod+k commandPalette.toggle');
  });

  it('keeps a rule that acts in a terminal as a clash with the keys of a session terminal, in both clients', () => {
    const taken = TAKEN_KEYS.filter((key) => key.place === 'session-terminal');
    expect(text(clashes(taken, () => [{ key: 'mod+shift+c', command: 'terminal.copy', when: 'terminalFocus' }]))).toEqual(['desktop other mod+shift+c terminal.copy', 'web other mod+shift+c terminal.copy']);
    expect(clashes(taken, () => [{ key: 'mod+shift+c', command: 'thread.copyReference', when: '!terminalFocus' }])).toEqual([]);
  });

  it('finds a clash with a key it is given', () => {
    const found = clashes([{ key: 'ctrl+shift+e', place: 'session-tab', does: 'A key', only: 'other' }], () => [{ key: 'mod+shift+e', command: 'composer.effort' }]);
    expect(text(found)).toEqual(['desktop other mod+shift+e composer.effort', 'web other mod+shift+e composer.effort']);
  });
});
