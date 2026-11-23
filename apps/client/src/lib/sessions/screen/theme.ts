import type { ITheme } from '@xterm/xterm';

/** The mapping in `design/tokens/terminal.tokens.json` (RA-010). */
export const ANSI_NORD = [1, 11, 14, 13, 9, 15, 8, 5, 3, 11, 14, 13, 9, 15, 7, 6] as const;
const BACKGROUND_NORD = 0;
const FOREGROUND_NORD = 4;
/** The cursor is nord4 like the text; the selection is nord3 under it, which keeps the text readable. */
const CURSOR_NORD = 4;
const SELECTION_NORD = 3;
const SELECTION_INACTIVE_NORD = 2;
/** Selected text, and the active find match, is nord6 on nord3: 6.4 to 1, whatever colour the text had. */
const SELECTION_TEXT_NORD = 6;

/** WCAG AA for text (UI-002); xterm.js lifts only the ANSI colours that fail on nord0. */
export const MINIMUM_CONTRAST_RATIO = 4.5;

export type TokenReader = (name: string) => string;

/** Every colour is a CSS variable read at the time of the call, so no colour value is written in code (RA-010, TOK-003). */
export function terminalTheme(read: TokenReader): ITheme {
  const nord = (index: number) => read(`--nord-${String(index)}`);
  return {
    background: nord(BACKGROUND_NORD),
    foreground: nord(FOREGROUND_NORD),
    cursor: nord(CURSOR_NORD),
    cursorAccent: nord(BACKGROUND_NORD),
    selectionBackground: nord(SELECTION_NORD),
    selectionForeground: nord(SELECTION_TEXT_NORD),
    selectionInactiveBackground: nord(SELECTION_INACTIVE_NORD),
    black: nord(ANSI_NORD[0]),
    red: nord(ANSI_NORD[1]),
    green: nord(ANSI_NORD[2]),
    yellow: nord(ANSI_NORD[3]),
    blue: nord(ANSI_NORD[4]),
    magenta: nord(ANSI_NORD[5]),
    cyan: nord(ANSI_NORD[6]),
    white: nord(ANSI_NORD[7]),
    brightBlack: nord(ANSI_NORD[8]),
    brightRed: nord(ANSI_NORD[9]),
    brightGreen: nord(ANSI_NORD[10]),
    brightYellow: nord(ANSI_NORD[11]),
    brightBlue: nord(ANSI_NORD[12]),
    brightMagenta: nord(ANSI_NORD[13]),
    brightCyan: nord(ANSI_NORD[14]),
    brightWhite: nord(ANSI_NORD[15]),
  };
}

export function tokensOf(element: Element): TokenReader {
  const style = getComputedStyle(element);
  return (name) => style.getPropertyValue(name).trim();
}

export function sizeInPx(token: string, rootPx: number, fallback: number): number {
  const match = /^(\d*\.?\d+)(rem|px)$/.exec(token);
  if (match === null) return fallback;
  const value = Number(match[1]);
  return match[2] === 'rem' ? value * rootPx : value;
}
