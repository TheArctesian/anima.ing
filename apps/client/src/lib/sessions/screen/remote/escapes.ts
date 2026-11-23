import type { Terminal } from '@xterm/xterm';

/** The longest title the tab shows, in characters (K4 §8.7). */
export const TITLE_MAX = 80;

const OSC_CLIPBOARD = 52;

/** `\p{Cf}` covers the bidirectional marks, overrides, isolates and zero-width characters. */
const UNSAFE_IN_TITLE = /[\p{Cc}\p{Cf}\u2028\u2029]/gu;

/** Cuts by code point, so an emoji is never cut (K4 §8.7). */
export function cleanTitle(raw: string): string {
  return [...raw.replace(UNSAFE_IN_TITLE, '').trim()].slice(0, TITLE_MAX).join('').trim();
}

export interface EscapeHooks {
  readonly ontitle: (title: string) => void;
  /** An OSC 8 link was activated; the address is the remote's, unchecked. */
  readonly onlink: (address: string) => void;
}

/** A remote cannot fill the clipboard, and an OSC 8 link goes to `onlink` and nowhere else (K4 §8.7). */
export function guardEscapes(terminal: Terminal, hooks: EscapeHooks): void {
  terminal.parser.registerOscHandler(OSC_CLIPBOARD, () => true);
  terminal.onTitleChange((title) => {
    hooks.ontitle(cleanTitle(title));
  });
  terminal.options.linkHandler = {
    activate: (_event, address) => {
      hooks.onlink(address);
    },
  };
}
