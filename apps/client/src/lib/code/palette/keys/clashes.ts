import { onPlatform, parseChord, type Chord, type KeyRule } from './keymap';
import { keyRules, type KeySet } from './web';

/** Where an existing shortcut acts, so a clash says whether both can reach the person at once. */
export type KeyPlace = 'threads-page' | 'session-terminal' | 'session-tab' | 'mac-app-menu' | 'browser' | 'windows';

/** A shortcut that Anima, its window or the browser already takes, in the syntax of the key table. */
export interface TakenKey {
  readonly key: string;
  readonly place: KeyPlace;
  readonly does: string;
  /** Mac only, or every platform but the Mac, when the shortcut is only there. */
  readonly only?: 'mac' | 'other';
  /** The command of the key table that does the same, so the two agree and do not clash. */
  readonly same?: string;
  /** Why a clash with this key stays: the ruling that keeps T3 Code's key over it. */
  readonly kept?: string;
}

/**
 * The shortcuts taken before the Code section, read from the code on 2026-11-21: the page of the threads
 * (`$lib/agent-threads/keys.ts`, THR-016), the operator's own keys of a session terminal
 * (`$lib/sessions/screen/operator/shortcuts.ts`, RA-003) and of a session tab, the macOS menu that Tauri gives a
 * window that sets none, and the keys a browser keeps for itself when the client runs as a web page.
 */
/** The ruling of 2026-11-21 on the clashes ("Remap for web"): the desktop app keeps T3 Code's keys. */
const KEPT_IN_DESKTOP =
  "The desktop app keeps T3 Code's key (operator, 2026-11-21): it closes the focused panel or terminal, else the window, and undoes in the chat when no text field has the focus";

/** A clash found after the ruling, in a key a page may take from the browser: listed for the next ruling. */
const OPEN_FOR_RULING =
  "Not in the ruled list, and a page may take this key from the browser (Anima's threads page already takes ⌘F). Open: remap it too, or keep it";

/** Ctrl+Alt in the desktop app on Windows, where it is AltGr: the ruling keeps T3 Code's key, which also types there. */
const OPEN_ON_WINDOWS =
  "The desktop app keeps T3 Code's key (operator, 2026-11-21), but on Windows Ctrl+Alt is AltGr and types a letter. Open: remap it in the Windows desktop app too, or keep it";

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
  { key: 'meta+w', place: 'mac-app-menu', does: 'Close the window', only: 'mac', kept: KEPT_IN_DESKTOP },
  { key: 'meta+m', place: 'mac-app-menu', does: 'Minimise the window', only: 'mac' },
  { key: 'meta+h', place: 'mac-app-menu', does: 'Hide Anima', only: 'mac' },
  { key: 'meta+z', place: 'mac-app-menu', does: 'Undo (Edit menu)', only: 'mac', kept: KEPT_IN_DESKTOP },
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
  { key: 'meta+shift+[', place: 'browser', does: 'Previous browser tab', only: 'mac' },
  { key: 'meta+shift+]', place: 'browser', does: 'Next browser tab', only: 'mac' },
  { key: 'mod+shift+o', place: 'browser', does: 'Bookmark manager (Chrome)' },
  { key: 'mod+shift+b', place: 'browser', does: 'Show or hide the bookmarks bar' },
  { key: 'mod+shift+p', place: 'browser', does: 'New private window (Firefox)' },
  { key: 'mod+r', place: 'browser', does: 'Reload the page' },
  { key: 'mod+=', place: 'browser', does: 'Zoom in' },
  { key: 'mod+-', place: 'browser', does: 'Zoom out' },
  { key: 'mod+0', place: 'browser', does: 'Reset the zoom' },
  { key: 'f6', place: 'browser', does: 'Go to the address bar' },
  { key: 'ctrl+shift+c', place: 'browser', does: 'Inspect an element (developer tools)', only: 'other' },
  { key: 'ctrl+shift+j', place: 'browser', does: 'Console (Chrome developer tools)', only: 'other' },
  { key: 'ctrl+shift+k', place: 'browser', does: 'Console (Firefox developer tools)', only: 'other' },
  { key: 'shift+f6', place: 'browser', does: 'Move the focus back among the parts of the window' },
  { key: 'meta+shift+c', place: 'browser', does: 'Inspect an element (Chrome developer tools)', only: 'mac' },
  { key: 'meta+shift+j', place: 'browser', does: 'Downloads (Chrome)', only: 'mac' },
  { key: 'alt+shift+t', place: 'browser', does: 'Go to the toolbar (Chrome)', only: 'other' },
  { key: 'alt+shift+b', place: 'browser', does: 'Go to the bookmarks bar (Chrome)', only: 'other' },
  { key: 'alt+shift+i', place: 'browser', does: 'Send feedback (Chrome)', only: 'other' },
  { key: 'alt+shift+a', place: 'browser', does: 'Go to an open dialog of the browser (Chrome)', only: 'other' },
  { key: 'meta+alt+b', place: 'browser', does: 'Bookmark manager (Chrome)', only: 'mac' },
  { key: 'mod+f', place: 'browser', does: 'Find in the page', kept: OPEN_FOR_RULING },
  { key: 'mod+shift+g', place: 'browser', does: 'Find the previous match', kept: OPEN_FOR_RULING },
  { key: 'mod+shift+a', place: 'browser', does: 'Search the tabs (Chrome)', kept: OPEN_FOR_RULING },
  { key: 'mod+shift+d', place: 'browser', does: 'Bookmark all tabs (Chrome)', kept: OPEN_FOR_RULING },
  { key: 'mod+b', place: 'browser', does: 'Bookmarks sidebar (Firefox)', kept: OPEN_FOR_RULING },
  { key: 'meta+k', place: 'browser', does: 'Search the web (Firefox)', only: 'mac', kept: OPEN_FOR_RULING },
  { key: 'ctrl+shift+m', place: 'browser', does: 'Profile menu (Chrome)', only: 'other', kept: OPEN_FOR_RULING },
  { key: 'ctrl+shift+s', place: 'browser', does: 'Screenshot (Firefox), web capture (Edge)', only: 'other', kept: OPEN_FOR_RULING },
  { key: 'ctrl+alt+n', place: 'windows', does: 'Start NVDA, the screen reader; as AltGr+N it also types ń on a Polish layout', only: 'other', kept: OPEN_ON_WINDOWS },
  { key: 'ctrl+alt+b', place: 'windows', does: 'As AltGr+B it types { on a Hungarian layout', only: 'other', kept: OPEN_ON_WINDOWS },
];

/** One shortcut of the key table that a taken shortcut also uses, on one platform. */
export interface Clash {
  readonly key: string;
  readonly platform: 'mac' | 'other';
  /** The desktop app, with T3 Code's keys, or the web client, with the web keys. */
  readonly client: 'desktop' | 'web';
  readonly command: string;
  readonly when?: string;
  readonly taken: TakenKey;
}

function same(left: Chord, right: Chord): boolean {
  return left.key === right.key && left.meta === right.meta && left.ctrl === right.ctrl && left.shift === right.shift && left.alt === right.alt;
}

/**
 * Whether a rule and a taken key can both act on one key press. A rule that acts only outside a terminal (`!terminalFocus`) or only in the browser panel (`previewFocus`) leaves the
 * keys of a session terminal alone; a rule that acts in a terminal can meet them.
 */
function meet(rule: KeyRule, taken: TakenKey): boolean {
  const when = rule.when ?? '';
  return !(taken.place === 'session-terminal' && (when.includes('!terminalFocus') || /(^|[^!])previewFocus/.test(when)));
}

/** The places a client meets: the desktop app has the Mac menu and no browser, the web client the reverse. */
function reaches(set: KeySet, taken: TakenKey): boolean {
  return set.web ? taken.place !== 'mac-app-menu' : taken.place !== 'browser';
}

/** The rules of one key set whose key a taken shortcut also uses with a different meaning. */
function clashesOf(set: KeySet, rules: readonly KeyRule[], taken: readonly TakenKey[]): Clash[] {
  const platform = set.mac ? 'mac' : 'other';
  const local = (key: string): Chord | null => {
    const chord = parseChord(key);
    return chord === null ? null : onPlatform(chord, set.mac);
  };
  const found: Clash[] = [];
  for (const key of taken) {
    const other = local(key.key);
    if (other === null || (key.only !== undefined && key.only !== platform) || !reaches(set, key)) continue;
    const sharing = rules.filter((rule) => {
      const chord = local(rule.key);
      return chord !== null && same(chord, other);
    });
    if (key.same !== undefined && sharing.some((rule) => rule.command === key.same)) continue;
    for (const rule of sharing.filter((candidate) => meet(candidate, key))) {
      found.push({ key: rule.key, platform, client: set.web ? 'web' : 'desktop', command: rule.command, ...(rule.when === undefined ? {} : { when: rule.when }), taken: key });
    }
  }
  return found;
}

/**
 * The clashes of the desktop app and of the web client, on the Mac and elsewhere: each rule whose key a taken shortcut
 * also uses with a different meaning. A taken key that some rule with the same key does the same as (`same`) agrees
 * with the table, so its key is no clash. `rulesOf` gives the rules of a key set.
 */
export function clashes(taken: readonly TakenKey[] = TAKEN_KEYS, rulesOf: (set: KeySet) => readonly KeyRule[] = keyRules): Clash[] {
  const sets: KeySet[] = [false, true].flatMap((web) => [true, false].map((mac) => ({ mac, web })));
  return sets.flatMap((set) => clashesOf(set, rulesOf(set), taken));
}
