import { describe, expect, it } from 'vitest';
import { clashes, TAKEN_KEYS } from './clashes';

const lines = (platform: 'mac' | 'other', place: string) =>
  clashes()
    .filter((clash) => clash.platform === platform && clash.taken.place === place)
    .map((clash) => `${clash.key} ${clash.command}`);

describe('the keys of T3 Code against the keys Anima already takes', () => {
  it('meets the menu of the Mac window on ⌘W and ⌘Z only', () => {
    expect(lines('mac', 'mac-app-menu')).toEqual(['mod+w terminal.close', 'mod+w rightPanel.close', 'mod+z thread.undo']);
  });

  it('agrees with the page it replaces, and leaves the keys of a session terminal alone', () => {
    expect(lines('mac', 'threads-page')).toEqual([]);
    expect(lines('other', 'threads-page')).toEqual([]);
    expect(lines('other', 'session-terminal')).toEqual([]);
  });

  it('meets the browser, as a web page, on the keys a browser keeps', () => {
    expect(lines('other', 'browser')).toContain('mod+k commandPalette.toggle');
    expect(lines('mac', 'browser')).not.toContain('mod+k commandPalette.toggle');
    expect(lines('mac', 'browser')).not.toContain('mod+1 thread.jump.1');
    expect(lines('mac', 'browser')).toContain('mod+n chat.new');
  });

  it('meets the tab keys of a Mac browser and the developer tools elsewhere', () => {
    expect(lines('mac', 'browser')).toEqual(expect.arrayContaining(['mod+shift+[ thread.previous', 'mod+shift+] thread.next', 'f6 workspace.focusNext', 'mod+r preview.refresh']));
    expect(lines('other', 'browser')).toEqual(expect.arrayContaining(['mod+shift+c thread.copyReference', 'mod+shift+j preview.toggle', 'mod+shift+k pullRequest.copyNumber']));
    expect(lines('mac', 'browser')).not.toContain('mod+shift+c thread.copyReference');
  });

  it('keeps a rule that acts in a terminal as a clash with the keys of a session terminal', () => {
    const taken = TAKEN_KEYS.filter((key) => key.place === 'session-terminal');
    expect(clashes([{ key: 'mod+shift+c', command: 'terminal.copy', when: 'terminalFocus' }], taken)).toHaveLength(1);
    expect(clashes([{ key: 'mod+shift+c', command: 'thread.copyReference', when: '!terminalFocus' }], taken)).toEqual([]);
  });

  it('finds a clash with a key it is given', () => {
    const found = clashes([{ key: 'mod+shift+e', command: 'composer.effort' }], [{ key: 'ctrl+shift+e', place: 'session-tab', does: 'A key', only: 'other' }, ...TAKEN_KEYS]);
    expect(found.map((clash) => [clash.platform, clash.command])).toEqual([['other', 'composer.effort']]);
  });
});
