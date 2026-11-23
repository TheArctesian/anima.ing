// Headscale reads `node.expiry` with Prometheus `model.ParseDuration`, and never expires a node on a value it rejects.

const second = 1000;
const minute = 60 * second;
const hour = 60 * minute;
const day = 24 * hour;

const unitMilliseconds = [365 * day, 7 * day, day, hour, minute, second, 1] as const;

// floor((2^63 - 1) ns / 1e6): the largest whole number of milliseconds Go can hold.
const maxMilliseconds = 9_223_372_036_854;

const grammar = /^(?:(\d+)y)?(?:(\d+)w)?(?:(\d+)d)?(?:(\d+)h)?(?:(\d+)m)?(?:(\d+)s)?(?:(\d+)ms)?$/;

/** Milliseconds, or `null` when Headscale cannot parse the text. */
export function parsePrometheusDuration(text: string): number | null {
  if (text === '0') {
    return 0;
  }
  const match = grammar.exec(text);
  if (text === '' || match === null) {
    return null;
  }
  let total = 0;
  unitMilliseconds.forEach((unit, index) => {
    const part = match[index + 1];
    if (part !== undefined) {
      total += Number(part) * unit;
    }
  });
  return total <= maxMilliseconds ? total : null;
}

export const days = (count: number): number => count * day;
