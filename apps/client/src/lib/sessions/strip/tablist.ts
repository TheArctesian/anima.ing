import type { TabId } from '../tabs';

export function panelId(id: TabId): string {
  return `session-panel-${id}`;
}

export function tabElementId(id: TabId): string {
  return `session-tab-${id}`;
}

/** APG tabs pattern, manual activation. */
export function nextFocus(key: string, current: number, count: number): number | null {
  if (count === 0) return null;
  switch (key) {
    case 'ArrowRight':
      return (current + 1) % count;
    case 'ArrowLeft':
      return (current - 1 + count) % count;
    case 'Home':
      return 0;
    case 'End':
      return count - 1;
    default:
      return null;
  }
}
