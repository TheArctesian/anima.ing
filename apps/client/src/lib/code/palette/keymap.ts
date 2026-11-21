/**
 * The keys of the Code section: T3 Code's default table (`packages/shared/src/keybindings.ts` at c6ca01b, MIT), in its
 * own syntax. `mod` is Command on a Mac and Ctrl elsewhere; a rule with `when` acts only where its condition holds.
 * The theme rules are left out: T3 Code's themes are not copied (R-352).
 */

export type KeyGroup = 'general' | 'threads' | 'composer' | 'terminal' | 'panels' | 'browser' | 'usage';

export interface KeyRule {
  readonly key: string;
  readonly command: string;
  readonly when?: string;
}

export const THREAD_JUMPS: readonly KeyRule[] = Array.from({ length: 9 }, (_, index) => ({
  key: `mod+${String(index + 1)}`,
  command: `thread.jump.${String(index + 1)}`,
  when: '!modelPickerOpen && isDesktop',
}));

export const MODEL_JUMPS: readonly KeyRule[] = Array.from({ length: 9 }, (_, index) => ({
  key: `mod+${String(index + 1)}`,
  command: `modelPicker.jump.${String(index + 1)}`,
  when: 'modelPickerOpen && isDesktop',
}));

/** T3 Code's defaults, in its order: when two rules match a key, the later one wins. */
export const KEY_RULES: readonly KeyRule[] = [
  { key: 'mod+b', command: 'sidebar.toggle' },
  { key: 'f6', command: 'workspace.focusNext' },
  { key: 'shift+f6', command: 'workspace.focusPrevious' },
  { key: 'mod+f6', command: 'workspace.focusNext' },
  { key: 'mod+shift+f6', command: 'workspace.focusPrevious' },
  { key: 'mod+[', command: 'navigation.back', when: '!terminalFocus' },
  { key: 'mod+]', command: 'navigation.forward', when: '!terminalFocus' },
  { key: 'mod+j', command: 'terminal.toggle' },
  { key: 'mod+alt+b', command: 'rightPanel.toggle' },
  { key: 'mod+shift+b', command: 'threadPanel.toggle', when: '!terminalFocus' },
  { key: 'mod+t', command: 'rightPanel.new', when: 'rightPanelOpen && !terminalFocus' },
  { key: 'mod+d', command: 'terminal.split', when: 'terminalFocus' },
  { key: 'mod+shift+d', command: 'terminal.splitVertical', when: 'terminalFocus' },
  { key: 'mod+shift+d', command: 'composer.dictation', when: '!terminalFocus' },
  { key: 'mod+n', command: 'terminal.new', when: 'terminalFocus' },
  { key: 'mod+w', command: 'terminal.close', when: 'terminalFocus' },
  { key: 'mod+w', command: 'rightPanel.close', when: '!terminalFocus' },
  { key: 'mod+shift+t', command: 'view.reopenClosed' },
  { key: 'mod+d', command: 'diff.toggle', when: '!terminalFocus' },
  { key: 'mod+shift+j', command: 'preview.toggle' },
  { key: 'mod+r', command: 'preview.refresh', when: 'previewFocus' },
  { key: 'mod+l', command: 'preview.focusUrl', when: 'previewFocus' },
  { key: 'mod+=', command: 'preview.zoomIn', when: 'previewFocus' },
  { key: 'mod++', command: 'preview.zoomIn', when: 'previewFocus' },
  { key: 'mod+-', command: 'preview.zoomOut', when: 'previewFocus' },
  { key: 'mod+0', command: 'preview.resetZoom', when: 'previewFocus' },
  { key: 'mod+f', command: 'preview.find', when: 'previewFocus' },
  { key: 'mod+k', command: 'commandPalette.toggle', when: '!terminalFocus' },
  { key: 'mod+p', command: 'filePicker.toggle', when: '!terminalFocus' },
  { key: 'mod+shift+f', command: 'projectSearch.toggle', when: '!terminalFocus' },
  { key: 'mod+u', command: 'usage.open', when: '!terminalFocus' },
  { key: 'mod+s', command: 'composer.stash', when: '!terminalFocus' },
  { key: 'mod+shift+enter', command: 'thread.steerQueuedMessage', when: '!terminalFocus' },
  { key: 'alt+arrowup', command: 'thread.editQueuedMessage', when: 'composerFocus' },
  { key: 'mod+enter', command: 'composer.sendAlternate', when: 'composerFocus && turnRunning' },
  { key: 'mod+enter', command: 'composer.sendBackground', when: 'composerFocus && draftThreadRoute' },
  { key: 'mod+alt+enter', command: 'composer.sendBackground', when: 'composerFocus && draftThreadRoute' },
  { key: 'mod+alt+enter', command: 'composer.sendAndNewThread', when: 'composerFocus && !draftThreadRoute' },
  { key: 'mod+n', command: 'chat.new', when: '!terminalFocus' },
  { key: 'mod+shift+o', command: 'chat.new', when: '!terminalFocus' },
  { key: 'mod+shift+n', command: 'chat.newLocal', when: '!terminalFocus' },
  { key: 'mod+alt+n', command: 'chat.newWithoutProject', when: '!terminalFocus' },
  { key: 'mod+f', command: 'chat.find', when: '!terminalFocus && !previewFocus' },
  { key: 'mod+shift+m', command: 'modelPicker.toggle', when: '!terminalFocus' },
  { key: 'mod+shift+h', command: 'composer.host', when: '!terminalFocus' },
  { key: 'mod+shift+e', command: 'composer.effort', when: '!terminalFocus' },
  { key: 'mod+shift+a', command: 'composer.mode', when: '!terminalFocus' },
  { key: 'mod+shift+x', command: 'composer.workspace', when: '!terminalFocus' },
  { key: 'mod+shift+g', command: 'composer.branch', when: '!terminalFocus' },
  { key: 'mod+shift+l', command: 'composer.previousWorktree', when: '!terminalFocus' },
  { key: 'mod+shift+k', command: 'pullRequest.copyNumber', when: '!terminalFocus' },
  { key: 'mod+shift+arrowup', command: 'modelPicker.previousProvider', when: 'modelPickerOpen' },
  { key: 'mod+shift+arrowdown', command: 'modelPicker.nextProvider', when: 'modelPickerOpen' },
  { key: 'mod+o', command: 'editor.openFavorite' },
  { key: 'mod+shift+[', command: 'thread.previous' },
  { key: 'mod+shift+]', command: 'thread.next' },
  { key: 'mod+shift+c', command: 'thread.copyReference', when: '!terminalFocus' },
  { key: 'mod+shift+s', command: 'thread.settle', when: '!terminalFocus' },
  { key: 'mod+shift+p', command: 'thread.pin', when: '!terminalFocus' },
  { key: 'mod+z', command: 'thread.undo', when: '!terminalFocus && !editableFocus' },
  ...THREAD_JUMPS,
  ...MODEL_JUMPS,
  { key: 'c', command: 'usage.cost', when: 'usagePageOpen' },
  { key: 't', command: 'usage.tokens', when: 'usagePageOpen' },
  { key: 'l', command: 'usage.limits', when: 'usagePageOpen' },
  { key: 'mod+shift+1', command: 'usage.period.day', when: 'usagePageOpen' },
  { key: 'mod+shift+2', command: 'usage.period.week', when: 'usagePageOpen' },
  { key: 'mod+shift+3', command: 'usage.period.month', when: 'usagePageOpen' },
  { key: 'mod+shift+4', command: 'usage.period.quarter', when: 'usagePageOpen' },
];

/** A key and its modifiers, as parsed from a rule. `mod` stands for Command or Ctrl until a platform is known. */
export interface Chord {
  readonly key: string;
  readonly mod: boolean;
  readonly meta: boolean;
  readonly ctrl: boolean;
  readonly shift: boolean;
  readonly alt: boolean;
}

type Modifier = 'mod' | 'meta' | 'ctrl' | 'shift' | 'alt';

/** The words of a rule that name a modifier, as T3 Code reads them. */
const MODIFIERS: Readonly<Record<string, Modifier>> = {
  mod: 'mod',
  cmd: 'meta',
  meta: 'meta',
  ctrl: 'ctrl',
  control: 'ctrl',
  shift: 'shift',
  alt: 'alt',
  option: 'alt',
};

/** A rule's key, `mod+shift+k`, as a chord; `null` when it is not one key with modifiers. */
export function parseChord(text: string): Chord | null {
  const tokens = text.toLowerCase().split('+');
  // `mod++` is the plus key: the split leaves two empty tokens at the end.
  if (tokens.at(-1) === '' && tokens.at(-2) === '') tokens.splice(-2, 2, '+');
  let key: string | null = null;
  const chord: Record<Modifier, boolean> = { mod: false, meta: false, ctrl: false, shift: false, alt: false };
  for (const token of tokens) {
    const modifier = MODIFIERS[token];
    if (modifier !== undefined) chord[modifier] = true;
    else if (token === '' || key !== null) return null;
    else key = token === 'esc' ? 'escape' : token;
  }
  return key === null ? null : { key, ...chord };
}

/** The chord on one platform: `mod` becomes Command on a Mac and Ctrl elsewhere. */
export function onPlatform(chord: Chord, mac: boolean): Chord {
  if (!chord.mod) return chord;
  return { ...chord, mod: false, meta: chord.meta || mac, ctrl: chord.ctrl || !mac };
}

const NAMED: Readonly<Record<string, string>> = {
  arrowup: '↑',
  arrowdown: '↓',
  arrowleft: '←',
  arrowright: '→',
  enter: 'Enter',
  escape: 'Esc',
  backspace: 'Backspace',
  delete: 'Delete',
  tab: 'Tab',
  ' ': 'Space',
};

function keyName(key: string): string {
  return NAMED[key] ?? (/^f\d{1,2}$/.test(key) ? key.toUpperCase() : key.length === 1 ? key.toUpperCase() : key);
}

/**
 * How a chord is written for the person: `⇧⌘K` on a Mac, in Apple's order of the marks (⌃ ⌥ ⇧ ⌘), and `Ctrl+Shift+K`
 * elsewhere.
 */
export function chordText(chord: Chord, mac: boolean): string {
  const local = onPlatform(chord, mac);
  const key = keyName(local.key);
  if (mac) {
    return `${local.ctrl ? '⌃' : ''}${local.alt ? '⌥' : ''}${local.shift ? '⇧' : ''}${local.meta ? '⌘' : ''}${key}`;
  }
  const parts = [local.ctrl ? 'Ctrl' : '', local.meta ? 'Meta' : '', local.alt ? 'Alt' : '', local.shift ? 'Shift' : '', key];
  return parts.filter((part) => part !== '').join('+');
}

/** The parts of a key event a chord is matched against. */
export interface Pressed {
  readonly key: string;
  readonly metaKey: boolean;
  readonly ctrlKey: boolean;
  readonly shiftKey: boolean;
  readonly altKey: boolean;
}

/** Whether a key press is the chord on this platform. Letters match in either case, so Shift+K is `shift+k`. */
export function pressedIs(pressed: Pressed, chord: Chord, mac: boolean): boolean {
  const local = onPlatform(chord, mac);
  return (
    pressed.key.toLowerCase() === local.key &&
    pressed.metaKey === local.meta &&
    pressed.ctrlKey === local.ctrl &&
    pressed.shiftKey === local.shift &&
    pressed.altKey === local.alt
  );
}
