/** The authenticator code is six digits (OD-2); the sign-in page shows one box for each. */
export const CODE_LENGTH = 6;

export type Digits = readonly string[];

export interface Entry {
  readonly digits: Digits;
  readonly focus: number;
}

export const EMPTY_DIGITS: Digits = Array.from({ length: CODE_LENGTH }, () => '');

export function digitsIn(text: string): string {
  return text.replace(/\D/g, '');
}

function lastBox(index: number): number {
  return Math.min(Math.max(index, 0), CODE_LENGTH - 1);
}

/** Six or more digits fill every box from the first: a pasted code or a one-time-code autofill. */
export function enter(digits: Digits, index: number, text: string): Entry {
  const found = digitsIn(text);
  if (found === '') return { digits, focus: index };
  if (found.length >= CODE_LENGTH) return { digits: [...found.slice(0, CODE_LENGTH)], focus: CODE_LENGTH - 1 };
  const next = [...digits];
  [...found].forEach((digit, offset) => {
    if (index + offset < CODE_LENGTH) next[index + offset] = digit;
  });
  return { digits: next, focus: lastBox(index + found.length) };
}

export function backspace(digits: Digits, index: number): Entry {
  const target = digits[index] === '' ? index - 1 : index;
  if (target < 0) return { digits, focus: 0 };
  return { digits: digits.map((digit, slot) => (slot === target ? '' : digit)), focus: target };
}

export function arrow(index: number, direction: -1 | 1): number {
  return lastBox(index + direction);
}

export function isComplete(digits: Digits): boolean {
  return digits.every((digit) => digit !== '');
}

export function codeOf(digits: Digits): string {
  return digits.join('');
}
