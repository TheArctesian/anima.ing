import { shellCapabilities } from '$lib/desktop/capabilities';
import { isMac } from '$lib/platform/mac';
import { isMobile } from '$lib/platform/mobile';
import { KEY_RULES, type KeyRule } from './keymap';

/**
 * The keys of the Code section in a browser ("Remap for web", operator, 2026-11-21). The desktop app keeps T3 Code's
 * table. In a browser, each T3 Code key of the ruled list that the browser keeps for itself on a platform, and each key
 * with Ctrl+Alt that Windows takes (Ctrl+Alt is AltGr there), becomes Shift with a letter and one more modifier: Alt on
 * Windows and Linux, Control on the Mac. Alt+Shift with a letter is free on Windows and Linux but for Chrome's
 * Alt+Shift+T, B, I and A, which are left out. On the Mac ⌥⇧ types letters on many layouts (Ą, Ł, Ż on Polish), while
 * ⌃⇧ types nothing, keeps the letter of the layout in the key event, and is not VoiceOver's ⌃⌥. The letter is
 * T3 Code's where it is free. `WEB_KEYS` writes each key with Alt; `webKeyOn` gives it for a platform.
 */

/** Which keys apply: the platform, and whether the client runs in a browser or in the desktop app. */
export interface KeySet {
  readonly mac: boolean;
  readonly web: boolean;
}

/** One T3 Code key that a browser keeps, and the key of the command in a browser. */
export interface WebKey {
  /** T3 Code's key and its command. */
  readonly from: string;
  readonly command: string;
  /** The key in a browser; `null` where the command keeps a different key of T3 Code's table. */
  readonly key: string | null;
  /** The platform where the browser keeps T3 Code's key, when it is only one. */
  readonly only?: 'mac' | 'other';
  /** What the browser does with T3 Code's key, and why the letter is a different one where it is. */
  readonly why: string;
}

export const WEB_KEYS: readonly WebKey[] = [
  { from: 'mod+n', command: 'chat.new', key: 'alt+shift+n', why: 'New browser window' },
  { from: 'mod+shift+o', command: 'chat.new', key: null, why: "Chrome's bookmark manager; New chat keeps its web key with N" },
  { from: 'mod+shift+n', command: 'chat.newLocal', key: 'alt+shift+h', why: 'New private window; N is New chat, so H for "here"' },
  { from: 'mod+alt+n', command: 'chat.newWithoutProject', key: 'alt+shift+f', only: 'other', why: 'Windows: starts NVDA, and AltGr+N types ń; N and H are taken, so F for "free of a project"' },
  { from: 'mod+n', command: 'terminal.new', key: 'alt+shift+n', why: 'New browser window' },
  { from: 'mod+w', command: 'terminal.close', key: 'alt+shift+w', why: 'Close the browser tab' },
  { from: 'mod+w', command: 'rightPanel.close', key: 'alt+shift+w', why: 'Close the browser tab' },
  { from: 'mod+t', command: 'rightPanel.new', key: 'alt+shift+y', why: "New browser tab; Alt+Shift+T is Chrome's toolbar, so Y, beside it" },
  { from: 'mod+shift+t', command: 'view.reopenClosed', key: 'alt+shift+z', why: 'Reopen the closed browser tab; Z as in undo' },
  { from: 'mod+d', command: 'diff.toggle', key: 'alt+shift+d', why: 'Bookmark the page' },
  { from: 'mod+d', command: 'terminal.split', key: 'alt+shift+d', why: 'Bookmark the page' },
  { from: 'mod+p', command: 'filePicker.toggle', key: 'alt+shift+p', why: 'Print' },
  { from: 'mod+s', command: 'composer.stash', key: 'alt+shift+s', why: 'Save the page' },
  { from: 'mod+o', command: 'editor.openFavorite', key: 'alt+shift+o', why: 'Open a file' },
  { from: 'mod+alt+b', command: 'rightPanel.toggle', key: 'alt+shift+g', why: "Chrome's bookmark manager on the Mac; AltGr+B types { on Windows; B is Chrome's, so G" },
  { from: 'mod+shift+b', command: 'threadPanel.toggle', key: 'alt+shift+v', why: "The bookmarks bar; Alt+Shift+B is Chrome's too, so V for view" },
  { from: 'mod+shift+p', command: 'thread.pin', key: 'alt+shift+m', why: "Firefox's private window; P is Go to file, so M for mark" },
  { from: 'mod+shift+c', command: 'thread.copyReference', key: 'alt+shift+c', why: 'Inspect an element (developer tools)' },
  { from: 'mod+shift+j', command: 'preview.toggle', key: 'alt+shift+e', why: 'Downloads (Chrome on the Mac), the console elsewhere; J is the terminal, so E' },
  { from: 'mod+l', command: 'preview.focusUrl', key: 'alt+shift+l', why: 'Go to the address bar' },
  { from: 'mod+r', command: 'preview.refresh', key: 'alt+shift+r', why: 'Reload the page' },
  { from: 'mod+=', command: 'preview.zoomIn', key: 'alt+shift+=', why: 'Zoom the page in' },
  { from: 'mod++', command: 'preview.zoomIn', key: null, why: 'Zoom the page in; Zoom in keeps its web key with =' },
  { from: 'mod+-', command: 'preview.zoomOut', key: 'alt+shift+-', why: 'Zoom the page out' },
  { from: 'mod+0', command: 'preview.resetZoom', key: 'alt+shift+0', why: 'Reset the zoom of the page' },
  { from: 'f6', command: 'workspace.focusNext', key: null, why: 'Go to the address bar; keeps ⌘F6 or Ctrl+F6' },
  { from: 'shift+f6', command: 'workspace.focusPrevious', key: null, why: "Move the browser's focus back; keeps ⇧⌘F6 or Ctrl+Shift+F6" },
  { from: 'mod+shift+[', command: 'thread.previous', key: 'alt+shift+[', only: 'mac', why: 'Previous browser tab' },
  { from: 'mod+shift+]', command: 'thread.next', key: 'alt+shift+]', only: 'mac', why: 'Next browser tab' },
  { from: 'mod+j', command: 'terminal.toggle', key: 'alt+shift+j', only: 'other', why: 'Downloads' },
  { from: 'mod+u', command: 'usage.open', key: 'alt+shift+u', only: 'other', why: 'View the source' },
  { from: 'mod+k', command: 'commandPalette.toggle', key: 'alt+shift+k', only: 'other', why: 'Search the web from the address bar' },
  { from: 'mod+shift+k', command: 'pullRequest.copyNumber', key: 'alt+shift+x', only: 'other', why: "Firefox's console; K is the palette, so X" },
];

/** A web key on one platform: Alt+Shift on Windows and Linux, Control+Shift (⌃⇧) on the Mac. */
export function webKeyOn(key: string, mac: boolean): string {
  return mac ? key.replace(/^alt\+shift\+/, 'ctrl+shift+') : key;
}

/** A rule for the desktop app alone (`isDesktop`): it never acts in a browser. */
function desktopOnly(rule: KeyRule): boolean {
  return /(^|[^!])isDesktop/.test(rule.when ?? '');
}

/** The rules of the key set: T3 Code's table in the desktop app, and in a browser the table with the web keys. */
export function keyRules(set: KeySet): KeyRule[] {
  if (!set.web) return [...KEY_RULES];
  const platform = set.mac ? 'mac' : 'other';
  return KEY_RULES.filter((rule) => !desktopOnly(rule)).flatMap((rule) => {
    const web = WEB_KEYS.find((candidate) => candidate.from === rule.key && candidate.command === rule.command && (candidate.only ?? platform) === platform);
    if (web === undefined) return [rule];
    return web.key === null ? [] : [{ ...rule, key: webKeyOn(web.key, set.mac) }];
  });
}

/**
 * The key set of this client: the platform it runs on, and the web keys unless the desktop shell runs it. The mobile shell
 * takes the web keys too: it has no desktop window, so T3 Code's rules for the desktop app (`isDesktop`) do not apply.
 */
export function thisKeySet(): KeySet {
  if (typeof navigator === 'undefined') return { mac: false, web: true };
  return { mac: isMac(navigator.userAgent), web: shellCapabilities() === null || isMobile(navigator) };
}
