const KEPT = '\t\n\r';

/** Controls are removed because an ESC could end the shell's bracketed paste and run the rest as typed commands (RA-003). */
export function safePaste(text: string): string {
  return text.replace(/\p{Cc}/gu, (character) => (KEPT.includes(character) ? character : ''));
}
