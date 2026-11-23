export type ShortcutAction = 'copy' | 'find' | 'paste';

export interface ShortcutKey {
  readonly key: string;
  readonly ctrlKey: boolean;
  readonly metaKey: boolean;
  readonly shiftKey: boolean;
  readonly altKey: boolean;
}

/**
 * RA-003. Ctrl+C alone is the interrupt and Ctrl+F alone the shell's "forward one character", so both reach the device;
 * paste is left to the browser's `paste` event, which xterm.js turns into bracketed paste.
 */
export function shortcutOf(event: ShortcutKey): ShortcutAction | undefined {
  if (event.altKey) return undefined;
  const key = event.key.toLowerCase();
  const chorded = event.ctrlKey && event.shiftKey && !event.metaKey;
  const command = event.metaKey && !event.ctrlKey && !event.shiftKey;
  if (key === 'c' && (chorded || command)) return 'copy';
  if (key === 'v' && (chorded || command)) return 'paste';
  if (key === 'f' && (chorded || command)) return 'find';
  return undefined;
}
