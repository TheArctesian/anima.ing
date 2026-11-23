// Go `time.ParseDuration`, for the Headscale durations read with `viper.GetDuration`; `node.expiry` uses prometheus-duration.ts.

const nanoseconds: Record<string, number> = {
  h: 3_600_000_000_000,
  m: 60_000_000_000,
  s: 1_000_000_000,
  ms: 1_000_000,
  us: 1000,
  'µs': 1000,
  'μs': 1000,
  ns: 1,
};

const element = /(\d*(?:\.\d*)?)(ms|us|µs|μs|ns|h|m|s)/y;

/** Nanoseconds, or `null` when Go cannot parse the text. */
export function parseGoDuration(text: string): number | null {
  const sign = text.startsWith('-') ? -1 : 1;
  const body = /^[-+]/.test(text) ? text.slice(1) : text;
  if (body === '0') {
    return 0;
  }
  if (body === '') {
    return null;
  }
  let total = 0;
  element.lastIndex = 0;
  while (element.lastIndex < body.length) {
    const match = element.exec(body);
    const [, number = '', unit = ''] = match ?? [];
    if (match === null || !/\d/.test(number)) {
      return null;
    }
    total += Number(number) * (nanoseconds[unit] ?? Number.NaN);
  }
  return Number.isFinite(total) && total < 2 ** 63 ? sign * total : null;
}

export const seconds = (count: number): number => count * 1_000_000_000;
