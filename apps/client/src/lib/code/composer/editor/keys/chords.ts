import type { Edit, Mode, Span } from '../edit';
import type { ChangeKind } from '../history/history';
import type { Style } from '../markdown/inline';
import { broken, tabbed } from './breaks';
import { toggled } from './pairs';

/** What a key does to the prompt: a change with its kind for undo, a step of history, or nothing of ours. */
export type Action = { readonly edit: Edit; readonly kind: ChangeKind } | 'undo' | 'redo' | null;

/** The part of a key press the editor reads. `mac` says which key is Mod: Command there, Control elsewhere. */
export interface Chord {
  readonly key: string;
  readonly shiftKey: boolean;
  readonly altKey: boolean;
  readonly ctrlKey: boolean;
  readonly metaKey: boolean;
  readonly mac: boolean;
}

/** The prompt as a key finds it. */
export interface Page {
  readonly value: string;
  readonly sel: Span;
  readonly mode: Mode;
}

// Mod-b, Mod-i, Mod-Shift-s, Mod-e (03, The editor: Rich features).
const STYLES: Record<string, Style | undefined> = { b: 'bold', i: 'italic', S: 'strike', e: 'code' };

function modChord(chord: Chord, page: Page): Action {
  const key = chord.key.toLowerCase();
  if (key === 'z') return chord.shiftKey ? 'redo' : 'undo';
  if (key === 'y' && !chord.mac && !chord.shiftKey) return 'redo';
  const style = STYLES[chord.shiftKey ? key.toUpperCase() : key];
  if (style === undefined || page.mode !== 'rich') return null;
  const change = toggled(page.value, page.sel, style);
  return change === null ? null : { edit: change, kind: 'other' };
}

/**
 * What a key press does, when the composer did not take it (03, The editor: Keyboard behaviour). Typing and the
 * delete keys are not here: they come as input events.
 */
export function chordAction(chord: Chord, page: Page): Action {
  const mod = chord.mac ? chord.metaKey && !chord.ctrlKey : chord.ctrlKey && !chord.metaKey;
  if (mod) return chord.altKey ? null : modChord(chord, page);
  if (chord.altKey || chord.ctrlKey || chord.metaKey) return null;
  if (chord.key === 'Enter') {
    const line = broken(page.value, page.sel, page.mode);
    return { edit: line.edit, kind: line.plain ? 'insert' : 'other' };
  }
  if (chord.key !== 'Tab') return null;
  const change = tabbed(page.value, page.sel, page.mode, chord.shiftKey);
  return change === null ? null : { edit: change, kind: 'other' };
}

const HELD: readonly string[] = ['Shift', 'Control', 'Alt', 'Meta'];

/**
 * Whether the next Tab is left to the browser, which moves focus with it: so after Escape where Tab or Shift+Tab
 * is the editor's, as in a code block and on a list line, and a keyboard could not leave the editor (WCAG 2.1.2).
 * `armed` is the answer the key before gave. A modifier alone keeps it, so Shift+Tab leaves too; each other key
 * ends it.
 */
export function leavesAfter(armed: boolean, key: string, page: Page): boolean {
  if (HELD.includes(key)) return armed;
  if (key !== 'Escape') return false;
  return tabbed(page.value, page.sel, page.mode, false) !== null || tabbed(page.value, page.sel, page.mode, true) !== null;
}
