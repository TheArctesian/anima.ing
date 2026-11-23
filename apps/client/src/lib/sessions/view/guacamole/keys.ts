/** X11 keysyms the Keys menu and the paste send (X11 `keysymdef.h`, as guacd reads them). */
export const KEYSYM = {
  controlLeft: 0xffe3,
  altLeft: 0xffe9,
  superLeft: 0xffeb,
  delete: 0xffff,
  escape: 0xff1b,
  tab: 0xff09,
  f4: 0xffc1,
  print: 0xff61,
  v: 0x76,
} as const;

/** The Meta and Super keys (Command on macOS): shortcuts with them belong to the operator's system, not the device. */
const SYSTEM_MODIFIERS: ReadonlySet<number> = new Set([0xffe7, 0xffe8, 0xffeb, 0xffec]);

const CONTROL: ReadonlySet<number> = new Set([0xffe3, 0xffe4]);
const SHIFT: ReadonlySet<number> = new Set([0xffe1, 0xffe2]);
const V: ReadonlySet<number> = new Set([0x56, 0x76]);
const INSERT = 0xff63;

/** One key event: 1 is a press, 0 a release (the values `sendKeyEvent` takes). */
export type KeyStroke = readonly [pressed: 0 | 1, keysym: number];

export interface KeyCombination {
  readonly id: string;
  readonly label: string;
  readonly hint: string;
  readonly keysyms: readonly number[];
}

/** The combinations the operator's own system keeps or that a laptop lacks, in menu order (RA-006). */
export const KEY_COMBINATIONS: readonly KeyCombination[] = [
  {
    id: 'ctrl-alt-delete',
    label: 'Ctrl+Alt+Delete',
    hint: 'Secure attention',
    keysyms: [KEYSYM.controlLeft, KEYSYM.altLeft, KEYSYM.delete],
  },
  { id: 'alt-tab', label: 'Alt+Tab', hint: 'Switch windows', keysyms: [KEYSYM.altLeft, KEYSYM.tab] },
  { id: 'alt-f4', label: 'Alt+F4', hint: 'Close window', keysyms: [KEYSYM.altLeft, KEYSYM.f4] },
  { id: 'super', label: 'Windows key', hint: 'Start menu', keysyms: [KEYSYM.superLeft] },
  { id: 'print', label: 'Print Screen', hint: 'Screenshot', keysyms: [KEYSYM.print] },
  { id: 'esc', label: 'Esc', hint: 'Escape', keysyms: [KEYSYM.escape] },
  { id: 'tab', label: 'Tab', hint: 'Next field', keysyms: [KEYSYM.tab] },
];

export function keyStrokes(combination: KeyCombination): readonly KeyStroke[] {
  const presses = combination.keysyms.map((keysym): KeyStroke => [1, keysym]);
  const releases = [...combination.keysyms].reverse().map((keysym): KeyStroke => [0, keysym]);
  return [...presses, ...releases];
}

/** A Ctrl the device already holds is neither pressed again nor released, so it stays down (RA-005). */
export function pasteStrokes(held: ReadonlySet<number>): readonly KeyStroke[] {
  const holdsControl = [...CONTROL].some((key) => held.has(key));
  return keyStrokes({ id: 'paste', label: 'Paste', hint: 'Send clipboard', keysyms: [...(holdsControl ? [] : [KEYSYM.controlLeft]), KEYSYM.v] });
}

export const PASTE_STROKES: readonly KeyStroke[] = pasteStrokes(new Set());

export interface KeyFilter {
  down(keysym: number): boolean;
  up(keysym: number): boolean;
  readonly held: ReadonlySet<number>;
}

function isPasteShortcut(keysym: number, held: ReadonlySet<number>): boolean {
  const any = (keys: ReadonlySet<number>) => [...keys].some((key) => held.has(key));
  if (keysym === INSERT) return any(SHIFT);
  return V.has(keysym) && any(CONTROL);
}

/**
 * With `paste` on, the paste shortcuts go to the browser, whose `paste` event sends the system clipboard; with it off,
 * the device pastes its own clipboard (RA-033).
 */
export function keyFilter(options: { readonly paste: boolean }): KeyFilter {
  const system = new Set<number>();
  const sent = new Set<number>();
  return {
    down: (keysym) => {
      if (SYSTEM_MODIFIERS.has(keysym)) system.add(keysym);
      if (system.size > 0 || (options.paste && isPasteShortcut(keysym, sent))) return false;
      sent.add(keysym);
      return true;
    },
    up: (keysym) => {
      system.delete(keysym);
      return sent.delete(keysym);
    },
    held: sent,
  };
}
