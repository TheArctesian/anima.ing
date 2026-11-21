import { describe, expect, it } from 'vitest';
import { chordText, KEY_RULES, onPlatform, parseChord } from './keymap';
import { shortcutOf } from './table';
import { keyRules, WEB_KEYS, type KeySet } from './web';

const MAC_APP: KeySet = { mac: true, web: false };
const MAC_WEB: KeySet = { mac: true, web: true };
const PC_WEB: KeySet = { mac: false, web: true };

const keyOf = (set: KeySet, command: string) => shortcutOf(command, set.mac, keyRules(set));

describe('the keys of the desktop app and of the web client', () => {
  it("keeps T3 Code's table in the desktop app", () => {
    expect(keyRules(MAC_APP)).toEqual(KEY_RULES);
    expect(keyOf(MAC_APP, 'chat.new')).toBe('⌘N');
  });

  it('gives a browser Alt+Shift keys where it keeps the T3 key, on the platform where it keeps it', () => {
    expect(keyOf(MAC_WEB, 'chat.new')).toBe('⌥⇧N');
    expect(keyOf(PC_WEB, 'chat.new')).toBe('Alt+Shift+N');
    expect(keyOf(MAC_WEB, 'commandPalette.toggle')).toBe('⌘K');
    expect(keyOf(PC_WEB, 'commandPalette.toggle')).toBe('Alt+Shift+K');
    expect(keyOf(MAC_WEB, 'thread.previous')).toBe('⌥⇧[');
    expect(keyOf(PC_WEB, 'thread.previous')).toBe('Ctrl+Shift+[');
  });

  it('leaves Ctrl+Alt to Windows in a browser, where it is AltGr, and to NVDA', () => {
    expect(keyOf(PC_WEB, 'chat.newWithoutProject')).toBe('Alt+Shift+F');
    expect(keyOf(MAC_WEB, 'chat.newWithoutProject')).toBe('⌥⌘N');
    expect(keyOf(PC_WEB, 'rightPanel.toggle')).toBe('Alt+Shift+G');
    expect(keyOf({ mac: false, web: false }, 'chat.newWithoutProject')).toBe('Ctrl+Alt+N');
  });

  it('leaves out the rules of the desktop app alone and the second keys a browser keeps', () => {
    const commands = keyRules(PC_WEB).map((rule) => `${rule.key} ${rule.command}`);
    expect(commands.some((line) => line.includes('thread.jump.'))).toBe(false);
    expect(commands).not.toContain('f6 workspace.focusNext');
    expect(commands).toContain('mod+f6 workspace.focusNext');
    expect(commands.filter((line) => line.endsWith(' chat.new'))).toEqual(['alt+shift+n chat.new']);
  });

  it('gives each web key to one command in each condition, and to no key of T3 Code', () => {
    for (const set of [MAC_WEB, PC_WEB]) {
      const t3 = new Set(KEY_RULES.map((rule) => chordText(onPlatform(parseChord(rule.key) ?? { key: '', mod: false, meta: false, ctrl: false, shift: false, alt: false }, set.mac), set.mac)));
      const seen = new Map<string, string>();
      for (const rule of keyRules(set).filter((candidate) => candidate.key.startsWith('alt+shift+'))) {
        const chord = parseChord(rule.key);
        expect(chord).not.toBeNull();
        const id = `${rule.key}|${rule.when ?? ''}`;
        expect(seen.get(id), id).toBeUndefined();
        seen.set(id, rule.command);
        expect(t3.has(chordText(chord ?? { key: '', mod: false, meta: false, ctrl: false, shift: false, alt: false }, set.mac))).toBe(false);
      }
    }
  });

  it('names a key and command of T3 Code in each web key', () => {
    for (const web of WEB_KEYS) expect(KEY_RULES.some((rule) => rule.key === web.from && rule.command === web.command), `${web.from} ${web.command}`).toBe(true);
  });
});
