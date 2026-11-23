import type { ListEntry } from './look';

function deviceKeys<T>(entries: readonly ListEntry<T>[]): string {
  return entries
    .filter((entry) => entry.kind === 'device')
    .map((entry) => entry.key)
    .sort()
    .join('\n');
}

/** The shown order holds while the same devices are listed, so the row under the pointer stays; `null` when a device came or went. */
export function heldOrder<T>(shown: readonly ListEntry<T>[], next: readonly ListEntry<T>[]): readonly ListEntry<T>[] | null {
  if (deviceKeys(shown) !== deviceKeys(next)) return null;
  const byKey = new Map(next.map((entry) => [entry.key, entry]));
  return shown.map((entry) => byKey.get(entry.key) ?? entry);
}
