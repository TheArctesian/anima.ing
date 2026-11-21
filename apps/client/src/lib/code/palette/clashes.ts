import { KEY_RULES, onPlatform, parseChord, type Chord, type KeyRule } from './keymap';

/** Where an existing shortcut acts, so a clash says whether both can reach the person at once. */
export type KeyPlace = 'threads-page' | 'session-terminal' | 'session-tab' | 'mac-app-menu' | 'browser';

/** A shortcut that Anima, its window or the browser already takes, in the syntax of the key table. */
export interface TakenKey {
  readonly key: string;
  readonly place: KeyPlace;
  readonly does: string;
  /** Mac only, or every platform but the Mac, when the shortcut is only there. */
  readonly only?: 'mac' | 'other';
  /** The command of the key table that does the same, so the two agree and do not clash. */
  readonly same?: string;
}

/**
 * The shortcuts taken before the Code section, read from the code on 2026-11-21: the page of the threads
 * (`$lib/agent-threads/keys.ts`, THR-016), the operator's own keys of a session terminal
 * (`$lib/sessions/screen/operator/shortcuts.ts`, RA-003) and of a session tab, the macOS menu that Tauri gives a
 * window that sets none, and the keys a browser keeps for itself when the client runs as a web page.
 */
export const TAKEN_KEYS: readonly TakenKey[] = [
  { key: 'mod+shift+[', place: 'threads-page', does: 'Previous thread', same: 'thread.previous' },
  { key: 'mod+shift+]', place: 'threads-page', does: 'Next thread', same: 'thread.next' },
  ...Array.from({ length: 9 }, (_, index) => ({
    key: `mod+${String(index + 1)}`,
    place: 'threads-page' as const,
    does: `Thread ${String(index + 1)}`,
    same: `thread.jump.${String(index + 1)}`,
  })),
  { key: 'mod+f', place: 'threads-page', does: 'Find in this thread', same: 'chat.find' },
  { key: '/', place: 'threads-page', does: 'Search threads' },
  { key: 'shift+?', place: 'threads-page', does: 'All keys' },
  { key: 'e', place: 'threads-page', does: 'Open or close the tool calls' },
  { key: 'meta+c', place: 'session-terminal', does: 'Copy', only: 'mac' },
  { key: 'ctrl+shift+c', place: 'session-terminal', does: 'Copy' },
  { key: 'meta+v', place: 'session-terminal', does: 'Paste', only: 'mac' },
  { key: 'ctrl+shift+v', place: 'session-terminal', does: 'Paste' },
  { key: 'meta+f', place: 'session-terminal', does: 'Find', only: 'mac' },
  { key: 'ctrl+shift+f', place: 'session-terminal', does: 'Find' },
  { key: 'shift+f10', place: 'session-tab', does: 'Open the tab menu' },
  { key: 'meta+q', place: 'mac-app-menu', does: 'Quit Anima', only: 'mac' },
  { key: 'meta+w', place: 'mac-app-menu', does: 'Close the window', only: 'mac' },
  { key: 'meta+m', place: 'mac-app-menu', does: 'Minimise the window', only: 'mac' },
  { key: 'meta+h', place: 'mac-app-menu', does: 'Hide Anima', only: 'mac' },
  { key: 'meta+z', place: 'mac-app-menu', does: 'Undo (Edit menu)', only: 'mac' },
  { key: 'meta+shift+z', place: 'mac-app-menu', does: 'Redo (Edit menu)', only: 'mac' },
  { key: 'ctrl+meta+f', place: 'mac-app-menu', does: 'Enter full screen', only: 'mac' },
  { key: 'mod+n', place: 'browser', does: 'New browser window (a page cannot take it)' },
  { key: 'mod+t', place: 'browser', does: 'New browser tab (a page cannot take it)' },
  { key: 'mod+w', place: 'browser', does: 'Close the browser tab (a page cannot take it)' },
  { key: 'mod+shift+t', place: 'browser', does: 'Reopen the closed browser tab' },
  { key: 'mod+shift+n', place: 'browser', does: 'New private window (a page cannot take it)' },
  { key: 'mod+l', place: 'browser', does: 'Go to the address bar' },
  { key: 'mod+d', place: 'browser', does: 'Bookmark the page' },
  { key: 'mod+p', place: 'browser', does: 'Print' },
  { key: 'mod+s', place: 'browser', does: 'Save the page' },
  { key: 'mod+o', place: 'browser', does: 'Open a file' },
  { key: 'mod+j', place: 'browser', does: 'Downloads', only: 'other' },
  { key: 'mod+u', place: 'browser', does: 'View the source', only: 'other' },
  { key: 'mod+k', place: 'browser', does: 'Search the web from the address bar', only: 'other' },
  ...Array.from({ length: 9 }, (_, index) => ({ key: `mod+${String(index + 1)}`, place: 'browser' as const, does: `Browser tab ${String(index + 1)}` })),
  { key: 'mod+[', place: 'browser', does: 'Back', only: 'mac', same: 'navigation.back' },
  { key: 'mod+]', place: 'browser', does: 'Forward', only: 'mac', same: 'navigation.forward' },
];

/** One shortcut of the key table that a taken shortcut also uses, on one platform. */
export interface Clash {
  readonly key: string;
  readonly platform: 'mac' | 'other';
  readonly command: string;
  readonly when?: string;
  readonly taken: TakenKey;
}

function same(left: Chord, right: Chord): boolean {
  return left.key === right.key && left.meta === right.meta && left.ctrl === right.ctrl && left.shift === right.shift && left.alt === right.alt;
}

/**
 * Whether a rule and a taken key can both act on one key press. A rule for the desktop app alone never runs in a browser;
 * a rule that names the focus of a terminal or the browser panel leaves the keys of a session terminal alone.
 */
function meet(rule: KeyRule, taken: TakenKey): boolean {
  const when = rule.when ?? '';
  if (taken.place === 'browser' && when.includes('isDesktop')) return false;
  return !(taken.place === 'session-terminal' && /terminalFocus|previewFocus/.test(when));
}

/**
 * The rules of `rules` whose key a taken shortcut also uses with a different meaning, on the Mac and elsewhere. A taken
 * key that some rule with the same key does the same as (`same`) agrees with the table, so its key is no clash.
 */
export function clashes(rules: readonly KeyRule[] = KEY_RULES, taken: readonly TakenKey[] = TAKEN_KEYS): Clash[] {
  const found: Clash[] = [];
  for (const platform of ['mac', 'other'] as const) {
    const mac = platform === 'mac';
    const local = (key: string): Chord | null => {
      const chord = parseChord(key);
      return chord === null ? null : onPlatform(chord, mac);
    };
    for (const key of taken) {
      const other = local(key.key);
      if (other === null || (key.only !== undefined && key.only !== platform)) continue;
      const sharing = rules.filter((rule) => {
        const chord = local(rule.key);
        return chord !== null && same(chord, other);
      });
      if (key.same !== undefined && sharing.some((rule) => rule.command === key.same)) continue;
      for (const rule of sharing.filter((candidate) => meet(candidate, key))) {
        found.push({ key: rule.key, platform, command: rule.command, ...(rule.when === undefined ? {} : { when: rule.when }), taken: key });
      }
    }
  }
  return found;
}
