import type { LiveSession } from './api';

/** The control's name: the count is part of it, so a screen reader hears it (RA-034). */
export function liveLabel(count: number): string {
  return `Live sessions, ${String(count)}`;
}

export function startedTime(session: LiveSession): string {
  return new Date(session.startedAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

/** The Stop buttons form a ring, and the control is before the first (C-3). */
export type TabStop = { readonly in: number } | { readonly to: 'control' };

export function nextTabStop(at: number, count: number, back: boolean): TabStop {
  if (count === 0) return { to: 'control' };
  if (back) return at <= 0 ? { to: 'control' } : { in: at - 1 };
  return { in: at + 1 >= count ? 0 : at + 1 };
}
