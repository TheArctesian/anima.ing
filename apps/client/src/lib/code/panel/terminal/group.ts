/**
 * The terminals of one tab of the right panel (task TC5; report 05, Section 4). A tab holds one group: one terminal, or
 * up to four in a split. This file is the state of the group with no drawing and no shell: which terminals there are,
 * which is active, and how they are split.
 */
export type SplitDirection = 'horizontal' | 'vertical';

export interface TerminalPane {
  readonly id: string;
  /** The words of the terminal in the list and in the title of the tab. */
  readonly label: string;
}

export interface TerminalGroup {
  readonly terminals: readonly TerminalPane[];
  /** The terminal that takes the keys; none when the group is empty. */
  readonly active: string | null;
  /** Set by the first split and then fixed: side by side, or stacked. */
  readonly direction?: SplitDirection | undefined;
}

export const NO_TERMINALS: TerminalGroup = { terminals: [], active: null };

/** The most terminals in one group. */
export const MAX_TERMINALS = 4;

export function canSplit(group: TerminalGroup): boolean {
  return group.terminals.length > 0 && group.terminals.length < MAX_TERMINALS;
}

/** With `terminal` added and active. The first split sets the direction; a full group does not change. */
export function added(group: TerminalGroup, terminal: TerminalPane, direction: SplitDirection): TerminalGroup {
  if (group.terminals.length >= MAX_TERMINALS || group.terminals.some((each) => each.id === terminal.id)) return group;
  const split = group.terminals.length === 0 ? undefined : (group.direction ?? direction);
  return { terminals: [...group.terminals, terminal], active: terminal.id, direction: split };
}

export function activated(group: TerminalGroup, id: string): TerminalGroup {
  return group.terminals.some((each) => each.id === id) ? { ...group, active: id } : group;
}

/** With the terminal closed. When it was active, the one at its right is active next, else the one at its left. */
export function ended(group: TerminalGroup, id: string): TerminalGroup {
  const at = group.terminals.findIndex((each) => each.id === id);
  if (at === -1) return group;
  const terminals = group.terminals.filter((each) => each.id !== id);
  const active = group.active === id ? ((terminals[at] ?? terminals[at - 1])?.id ?? null) : group.active;
  return { terminals, active, direction: terminals.length > 1 ? group.direction : undefined };
}

/** The title of the tab: the words of the active terminal. */
export function titleOf(group: TerminalGroup): string {
  return group.terminals.find((each) => each.id === group.active)?.label ?? 'Terminal';
}
