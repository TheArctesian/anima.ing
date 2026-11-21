import type { CodeIconName } from '../icons/icons';

/** The states a chat shows in the inbox, as T3 Code names them (t3-ui-copy-2026-11-21/02 section 5.5). */
export type ChatStatus = 'preparing' | 'working' | 'waiting' | 'approval' | 'input' | 'request' | 'limited' | 'failed' | 'woke' | 'done';

/** The status token a state takes: decision D11 maps T3's colours to Anima's status tokens. */
export type StatusTone = 'accent' | 'degraded' | 'down' | 'up' | 'secondary';

export interface StatusLook {
  readonly label: string;
  readonly icon: CodeIconName;
  readonly tone: StatusTone;
}

/** Each state shows an icon and a word, never a colour alone. */
export const STATUS_LOOKS: Readonly<Record<ChatStatus, StatusLook>> = {
  preparing: { label: 'Setting up', icon: 'circle-dashed', tone: 'accent' },
  working: { label: 'Working', icon: 'circle-dashed', tone: 'accent' },
  waiting: { label: 'Waiting', icon: 'circle-dot-dashed', tone: 'secondary' },
  approval: { label: 'Approval', icon: 'shield-question', tone: 'degraded' },
  input: { label: 'Input', icon: 'message-circle-question', tone: 'degraded' },
  request: { label: 'Request', icon: 'user-round', tone: 'degraded' },
  limited: { label: 'Limited', icon: 'circle-alert', tone: 'degraded' },
  failed: { label: 'Failed', icon: 'circle-alert', tone: 'down' },
  woke: { label: 'Woke', icon: 'alarm-clock', tone: 'degraded' },
  done: { label: 'Done', icon: 'circle-check', tone: 'up' },
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
