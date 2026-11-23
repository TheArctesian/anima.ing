import type { IconName } from '$lib/icons/icons';

/** The label is always shown (UI-003). */
export interface TabSpec {
  readonly id: string;
  readonly label: string;
  readonly count?: number;
  readonly icon?: IconName;
}

export function tabIds(prefix: string, id: string): { tab: string; panel: string } {
  return { tab: `${prefix}-tab-${id}`, panel: `${prefix}-panel-${id}` };
}

interface Span {
  readonly start: number;
  readonly size: number;
}

export function scrolledToShow(view: Span, tab: Span, margin: number): number {
  if (tab.start - margin < view.start) return Math.max(0, tab.start - margin);
  const tabEnd = tab.start + tab.size + margin;
  return tabEnd > view.start + view.size ? tabEnd - view.size : view.start;
}
