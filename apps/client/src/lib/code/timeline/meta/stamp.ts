/**
 * The times of the timeline as T3 Code writes them (`formatDayAwareTimestamp`, fidelity 13): the time of day today,
 * "yesterday at {time}", then the date and the time; the year only when it is not this year.
 */

const DAY_MS = 86_400_000;

function dayNumber(date: Date): number {
  return Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()) / DAY_MS;
}

/** The short time of a message or a row. */
export function dayTime(at: Date, now: Date): string {
  const time = at.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  const days = dayNumber(now) - dayNumber(at);
  if (days <= 0) return time;
  if (days === 1) return `yesterday at ${time}`;
  const date = at.toLocaleDateString([], at.getFullYear() === now.getFullYear() ? { month: 'numeric', day: 'numeric' } : { year: 'numeric', month: 'numeric', day: 'numeric' });
  return `${date} ${time}`;
}

/** The whole date and time, for the tooltip of a time. */
export function fullDate(at: Date): string {
  return at.toLocaleString([], { dateStyle: 'full', timeStyle: 'medium' });
}
