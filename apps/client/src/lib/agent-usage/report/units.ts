// The units of `anima agents usage`, so the page and the command read the same (crates/anima-cli/src/agents/report.rs).

export function whole(count: number): string {
  return count.toFixed(0);
}

export function tokens(count: number): string {
  for (const [size, unit] of [
    [1e9, 'B'],
    [1e6, 'M'],
    [1e3, 'k'],
  ] as const) {
    if (count >= size) return `${(count / size).toFixed(1)}${unit}`;
  }
  return whole(count);
}

export function dollars(usd: number): string {
  return usd > 0 && usd < 0.005 ? '<0.01' : usd.toFixed(2);
}

/** The words of a cost that has no price (AIS-021). */
export const NO_PRICE = 'No price';
