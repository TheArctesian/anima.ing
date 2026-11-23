import type { MenuEntry } from '../../kit/menu';
import { MAX_TERMINALS, type SplitDirection, type TerminalGroup, type TerminalPane } from './group';

/** The words of the terminal surface, as T3 Code has them (report 05, Sections 4.5 to 4.10). */
export type TerminalAction = 'split_horizontal' | 'split_vertical' | 'new' | 'close';

export type TerminalKeys = Partial<Record<TerminalAction, string>>;

const ACTION_WORDS: Record<TerminalAction, string> = {
  split_horizontal: 'Split Terminal Horizontally',
  split_vertical: 'Split Terminal Vertically',
  new: 'New Terminal',
  close: 'Close Terminal',
};

/** The name of an action: its words, with its key, or with the reason that a split is off. */
export function actionLabel(action: TerminalAction, keys: TerminalKeys = {}, full = false): string {
  const words = ACTION_WORDS[action];
  if (full && (action === 'split_horizontal' || action === 'split_vertical')) return `${words} (max ${String(MAX_TERMINALS)} per group)`;
  const key = keys[action];
  return key === undefined ? words : `${words} (${key})`;
}

export function closeLabel(terminal: TerminalPane, key?: string): string {
  return key === undefined ? `Close ${terminal.label}` : `Close ${terminal.label} (${key})`;
}

/** The header of the list: how the group is split. */
export function groupWords(group: TerminalGroup): { readonly label: string; readonly direction: SplitDirection | undefined } {
  if (group.terminals.length < 2) return { label: 'Single', direction: undefined };
  return { label: group.direction === 'vertical' ? 'Stacked' : 'Side by side', direction: group.direction ?? 'horizontal' };
}

export const EMPTY_WORDS = 'No terminal sessions for this thread yet.';
export const NO_PERMISSION_WORDS = 'You do not have permission to operate terminals here.';

/** The words of a pane whose drawing did not start. */
export function failureWords(message = 'Unable to start the terminal'): string {
  return `${message}. Close and reopen the terminal to retry.`;
}

/** The question before terminals close: its title and its detail. */
export function closeQuestion(terminals: readonly TerminalPane[]): { readonly title: string; readonly detail: string } {
  const [only] = terminals;
  if (terminals.length === 1 && only !== undefined) {
    return { title: `Close terminal "${only.label}"?`, detail: 'This stops the running process and clears its history.' };
  }
  const labels = terminals.map((each) => each.label).join(', ');
  return { title: `Close ${String(terminals.length)} terminals?`, detail: `This stops their running processes and clears their histories: ${labels}.` };
}

export type PaneChoice = 'add_to_chat' | 'copy' | 'paste' | 'select_all' | 'jump_to_latest';

/** The right-click menu of a pane, in the order of T3 Code. */
export function paneMenu(pane: { readonly selection: boolean; readonly readOnly: boolean }): readonly MenuEntry[] {
  return [
    { kind: 'choice', id: 'add_to_chat', label: 'Add to chat', disabled: !pane.selection },
    { kind: 'choice', id: 'copy', label: 'Copy', disabled: !pane.selection },
    { kind: 'choice', id: 'paste', label: 'Paste', disabled: pane.readOnly },
    { kind: 'choice', id: 'select_all', label: 'Select all' },
    { kind: 'choice', id: 'jump_to_latest', label: 'Jump to latest' },
  ];
}
