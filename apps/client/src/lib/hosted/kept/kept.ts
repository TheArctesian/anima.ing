import { daysText } from '$lib/retention/periods';
import type { RetentionPeriod } from '$lib/retention/api';

/** What the hub keeps (HMD-073); a figure is `null` when the hub did not give it. */
export interface HubKept {
  readonly devices: number | null;
  readonly checks: number | null;
  readonly periods: readonly Pick<RetentionPeriod, 'subject' | 'days'>[] | null;
}

export const NOT_AVAILABLE = 'Not available just now';

/** The kinds of history the card names, in its order, with the operator's word for each (D-25). */
const HISTORY: readonly { readonly subject: RetentionPeriod['subject']; readonly words: string }[] = [
  { subject: 'METRICS', words: 'metrics' },
  { subject: 'HEARTBEATS', words: 'check results' },
];

export function countText(count: number | null): string {
  return count === null ? NOT_AVAILABLE : count.toLocaleString('en');
}

export function historyText(periods: HubKept['periods']): string {
  const named = HISTORY.flatMap(({ subject, words }) => {
    const period = periods?.find((one) => one.subject === subject);
    return period === undefined ? [] : [`${daysText(period.days)} of ${words}`];
  });
  return named.length === 0 ? NOT_AVAILABLE : named.join(', ');
}
