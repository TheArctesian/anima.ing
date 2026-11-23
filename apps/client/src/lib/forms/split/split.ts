import type { IconName } from '$lib/icons/icons';

export interface SplitChoice {
  readonly id: string;
  readonly label: string;
  /** The accessible name when the label alone is too short: "RDP to nas" for "RDP". */
  readonly spoken?: string;
  readonly icon?: IconName;
  readonly shortcut?: string;
}

export function nextIndex(key: string, current: number, count: number): number | null {
  if (count === 0) return null;
  if (key === 'ArrowDown') return (current + 1) % count;
  if (key === 'ArrowUp') return (current - 1 + count) % count;
  if (key === 'Home') return 0;
  if (key === 'End') return count - 1;
  return null;
}
