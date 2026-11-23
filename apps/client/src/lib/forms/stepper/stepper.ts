/** `empty` lets one step down from `min` clear the value (for "no limit"). */
export interface StepBounds {
  readonly min: number;
  readonly max: number;
  readonly empty: boolean;
}

export function stepped(text: string, by: 1 | -1, bounds: StepBounds): string {
  const typed = text.trim();
  const current = /^\d+$/.test(typed) ? Number(typed) : null;
  if (current === null) return by > 0 || !bounds.empty ? String(bounds.min) : '';
  const next = current + by;
  if (next < bounds.min) return bounds.empty ? '' : String(bounds.min);
  return String(Math.min(bounds.max, next));
}
