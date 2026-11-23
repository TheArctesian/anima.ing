const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;

function counted(count: number, unit: string): string {
  return count === 1 ? `1 ${unit} ago` : `${String(count)} ${unit}s ago`;
}

/** NET-004: whole units, rounded down, so the text changes at most once a minute. */
export function dataAge(since: Date, now: Date): string {
  const elapsed = now.getTime() - since.getTime();
  if (elapsed < MINUTE_MS) return 'just now';
  if (elapsed < HOUR_MS) return counted(Math.floor(elapsed / MINUTE_MS), 'minute');
  if (elapsed < DAY_MS) return counted(Math.floor(elapsed / HOUR_MS), 'hour');
  return counted(Math.floor(elapsed / DAY_MS), 'day');
}
