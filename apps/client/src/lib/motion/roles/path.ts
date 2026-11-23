export const opacity = (t: number) => `opacity: ${String(t)};`;

export function scaledFade(atZero: number | undefined, origin: string | undefined) {
  if (atZero === undefined) return opacity;
  const at = origin === undefined ? '' : ` transform-origin: ${origin};`;
  return (t: number) => `${opacity(t)} scale: ${String(atZero + (1 - atZero) * t)};${at}`;
}

/** A reversal mid-flight keeps the first direction's value, so the element goes back on its path (MOT-002, MOT-005). */
export function firstDirection<T>(): (own: () => T) => T {
  let first: { readonly value: T } | undefined;
  return (own) => (first ??= { value: own() }).value;
}
