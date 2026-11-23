export interface KeySequence {
  readonly id: string;
  readonly label: string;
  readonly hint: string;
  readonly bytes: Uint8Array<ArrayBuffer>;
}

const ESCAPE = 0x1b;
const TAB = 0x09;
const control = (letter: string) => (letter.charCodeAt(0) & 0x1f);

function sequence(id: string, label: string, hint: string, ...codes: number[]): KeySequence {
  return { id, label, hint, bytes: Uint8Array.from(codes) };
}

/** Ctrl+Alt+Delete is not offered: a shell has no such key (RA-006, RA-007). */
export const KEY_SEQUENCES: readonly KeySequence[] = [
  sequence('ctrl-c', 'Ctrl+C', 'Interrupt', control('c')),
  sequence('ctrl-d', 'Ctrl+D', 'End of input', control('d')),
  sequence('ctrl-z', 'Ctrl+Z', 'Suspend', control('z')),
  sequence('ctrl-l', 'Ctrl+L', 'Clear screen', control('l')),
  sequence('esc', 'Esc', 'Escape', ESCAPE),
  sequence('tab', 'Tab', 'Complete', TAB),
];
