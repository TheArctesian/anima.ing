import type { IconName } from '$lib/icons/icons';

/** The states a chat shows in the inbox, as T3 Code names them (t3-ui-copy-2026-11-21/02 section 5.5). */
export type ChatStatus = 'preparing' | 'working' | 'waiting' | 'approval' | 'input' | 'request' | 'limited' | 'failed' | 'woke' | 'done';

/** The status token a state takes: decision D11 maps T3's colours to Anima's status tokens. */
export type StatusTone = 'accent' | 'degraded' | 'down' | 'up' | 'secondary';

export interface StatusLook {
  readonly label: string;
  readonly icon: IconName;
  readonly tone: StatusTone;
}

/** Each state shows an icon and a word, never a colour alone. */
export const STATUS_LOOKS: Readonly<Record<ChatStatus, StatusLook>> = {
  preparing: { label: 'Setting up', icon: 'carbon:circle-dash', tone: 'accent' },
  working: { label: 'Working', icon: 'carbon:circle-dash', tone: 'accent' },
  waiting: { label: 'Waiting', icon: 'carbon:time', tone: 'secondary' },
  approval: { label: 'Approval', icon: 'carbon:security', tone: 'degraded' },
  input: { label: 'Input', icon: 'carbon:information', tone: 'degraded' },
  request: { label: 'Request', icon: 'carbon:user', tone: 'degraded' },
  limited: { label: 'Limited', icon: 'carbon:warning', tone: 'degraded' },
  failed: { label: 'Failed', icon: 'carbon:warning', tone: 'down' },
  woke: { label: 'Woke', icon: 'carbon:notification', tone: 'degraded' },
  done: { label: 'Done', icon: 'carbon:checkmark-outline', tone: 'up' },
};

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** The age of a chat with no status, as the inbox writes it: "now" under a minute, then "5m", "3h", "2d". */
export function shortAge(since: number, now: number): string {
  const age = Math.max(0, now - since);
  if (age < MINUTE) return 'now';
  if (age < HOUR) return `${String(Math.floor(age / MINUTE))}m`;
  if (age < DAY) return `${String(Math.floor(age / HOUR))}h`;
  return `${String(Math.floor(age / DAY))}d`;
}
