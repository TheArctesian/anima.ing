import type { CodeIconName } from '../icons/icons';

/**
 * A piece of the prompt that the editor draws as one chip: a mentioned file, a skill, a command. The prompt itself
 * is always the Markdown string (03, "The editor"); `start` and `end` are offsets in it.
 */
export interface Chip {
  readonly start: number;
  readonly end: number;
  readonly label: string;
  readonly icon: CodeIconName;
  /** The full words for the tip and for assistive technology: the whole path of a file. */
  readonly detail: string;
}

/** Finds the chips of a prompt. The editor draws what it returns and knows nothing of what a chip means. */
export type ChipReader = (prompt: string) => readonly Chip[];

/** Where the caret is: the prompt, the offset in it, and the caret's place on screen for a popup to anchor on. */
export interface CaretAt {
  readonly prompt: string;
  readonly caret: number;
  /** Null while the editor has no focus or a range is selected. */
  readonly rect: DOMRect | null;
}

/** Whether a part of the composer can be used now; when it cannot, the words say why, for the tip of the control. */
export type Can = { readonly can: true } | { readonly can: false; readonly reason: string };

export const CAN: Can = { can: true };

export function cannot(reason: string): Can {
  return { can: false, reason };
}
