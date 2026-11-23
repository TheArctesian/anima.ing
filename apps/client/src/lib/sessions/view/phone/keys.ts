/** The keys of the row over the on-screen keyboard in an SSH session (RA-050, MOB-020; mockup 8). */
export type PhoneKey =
  | { readonly kind: 'bytes'; readonly label: string; readonly name: string; readonly bytes: Uint8Array<ArrayBuffer> }
  | { readonly kind: 'modifier'; readonly label: string; readonly modifier: Modifier }
  | { readonly kind: 'gap' };

export type Modifier = 'ctrl' | 'alt';

export interface Latch {
  readonly ctrl: boolean;
  readonly alt: boolean;
}

export const NO_LATCH: Latch = { ctrl: false, alt: false };

const ESCAPE = 0x1b;
const text = (value: string) => Uint8Array.from(new TextEncoder().encode(value));
const key = (label: string, name: string, value: string): PhoneKey => ({ kind: 'bytes', label, name, bytes: text(value) });

/** In the order of mockup 8; the arrows send a terminal's normal-mode cursor keys. */
export const PHONE_KEYS: readonly PhoneKey[] = [
  key('Esc', 'Escape', '\u001b'),
  key('Tab', 'Tab', '\t'),
  { kind: 'modifier', label: 'Ctrl', modifier: 'ctrl' },
  { kind: 'modifier', label: 'Alt', modifier: 'alt' },
  { kind: 'gap' },
  key('←', 'Left', '\u001b[D'),
  key('↓', 'Down', '\u001b[B'),
  key('↑', 'Up', '\u001b[A'),
  key('→', 'Right', '\u001b[C'),
  { kind: 'gap' },
  key('|', 'Pipe', '|'),
  key('~', 'Tilde', '~'),
  key('/', 'Slash', '/'),
  key('-', 'Hyphen', '-'),
];

export function toggled(latch: Latch, modifier: Modifier): Latch {
  return { ...latch, [modifier]: !latch[modifier] };
}

export function latched(latch: Latch): boolean {
  return latch.ctrl || latch.alt;
}

/** Ctrl turns one byte into its control code and Alt puts Escape before it; input of more than one byte is sent as it is. */
export function withLatch(bytes: Uint8Array<ArrayBuffer>, latch: Latch): Uint8Array<ArrayBuffer> {
  if (!latched(latch) || bytes.length !== 1) return bytes;
  let code = bytes[0] ?? 0;
  if (latch.ctrl) code = controlCode(code);
  return latch.alt ? Uint8Array.from([ESCAPE, code]) : Uint8Array.from([code]);
}

function controlCode(code: number): number {
  if (code === 0x20) return 0;
  if (code >= 0x40 && code <= 0x7f) return code & 0x1f;
  return code;
}
