/** The keys of the page Code (THR-016), as a pure rule: a key and where the focus is give one action or none. */

export type KeyAction =
  | { readonly kind: 'move'; readonly by: 1 | -1 }
  | { readonly kind: 'jump'; readonly place: number }
  | { readonly kind: 'search' }
  | { readonly kind: 'find' }
  | { readonly kind: 'tools' }
  | { readonly kind: 'help' }
  | { readonly kind: 'leave' };

export interface Pressed {
  readonly key: string;
  /** Cmd on a Mac, Ctrl elsewhere. */
  readonly mod: boolean;
  readonly shift: boolean;
  readonly alt: boolean;
  readonly typing: boolean;
}

/** With Shift, a keyboard gives `{` and `}` for the bracket keys. */
function withMod(key: string, shift: boolean): KeyAction | null {
  if (shift) {
    if (key === '[' || key === '{') return { kind: 'move', by: -1 };
    return key === ']' || key === '}' ? { kind: 'move', by: 1 } : null;
  }
  if (/^[1-9]$/.test(key)) return { kind: 'jump', place: Number(key) - 1 };
  return key.toLowerCase() === 'f' ? { kind: 'find' } : null;
}

const PLAIN: Readonly<Record<string, KeyAction>> = {
  ArrowDown: { kind: 'move', by: 1 },
  ArrowUp: { kind: 'move', by: -1 },
  '/': { kind: 'search' },
  '?': { kind: 'help' },
  e: { kind: 'tools' },
  E: { kind: 'tools' },
};

export function keyAction(pressed: Pressed): KeyAction | null {
  const { key, mod, shift, alt, typing } = pressed;
  if (alt) return null;
  if (key === 'Escape') return typing ? { kind: 'leave' } : null;
  if (mod) return withMod(key, shift);
  return typing ? null : (PLAIN[key] ?? null);
}

/** `-1` means none chosen; a move stops at the ends. */
export function moved(current: number, by: 1 | -1, length: number): number {
  if (length === 0) return -1;
  if (current < 0) return by === 1 ? 0 : length - 1;
  return Math.min(length - 1, Math.max(0, current + by));
}

export const KEY_WORDS: readonly (readonly [string, string])[] = [
  ['⌘⇧[ ⌘⇧]', 'Previous and next thread'],
  ['⌘1 – ⌘9', 'Thread 1 to 9'],
  ['↑ ↓', 'Move in the list'],
  ['/', 'Search threads'],
  ['⌘F', 'Find in this thread'],
  ['E', 'Open or close the tool calls'],
  ['?', 'All keys'],
];
