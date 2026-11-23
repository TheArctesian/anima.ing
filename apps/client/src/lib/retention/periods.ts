import type { RetentionPeriod, RetentionSubject } from './api';

/** Settings lists the retained kinds in this order, with these names (D-25). */
export const SUBJECTS: readonly { readonly subject: RetentionSubject; readonly label: string }[] = [
  { subject: 'HEARTBEATS', label: 'Check results' },
  { subject: 'METRICS', label: 'Metrics' },
  { subject: 'EVENTS', label: 'Events' },
  { subject: 'DELIVERIES', label: 'Notification deliveries' },
];

export type DaysCheck = { readonly ok: true; readonly days: number } | { readonly ok: false; readonly message: string };

/** Nothing is sent when this refuses (HUB-005). */
export function checkDays(text: string, period: Pick<RetentionPeriod, 'minDays' | 'maxDays'>): DaysCheck {
  const trimmed = text.trim();
  if (!/^-?\d+$/.test(trimmed)) return { ok: false, message: 'Enter a whole number of days.' };
  const days = Number(trimmed);
  if (days < period.minDays || days > period.maxDays) {
    return { ok: false, message: `Enter ${String(period.minDays)} to ${String(period.maxDays)} days.` };
  }
  return { ok: true, days };
}

export function inPageOrder(periods: readonly RetentionPeriod[]): readonly RetentionPeriod[] {
  return SUBJECTS.flatMap(({ subject }) => periods.filter((period) => period.subject === subject));
}

export function daysText(days: number): string {
  return days === 1 ? '1 day' : `${String(days)} days`;
}
