import { DESKTOP_LIMITS, type CopyPlace } from './copy';
import { indexedCopyStore } from './indexed';

let era = 0;

/** A browser keeps no copy (FND-007): it shows nothing without the hub anyway, and it can be a shared computer. */
export function desktopRecentCopy(): CopyPlace | undefined {
  if (!('indexedDB' in globalThis)) return undefined;
  return { store: indexedCopyStore(globalThis.indexedDB), limits: DESKTOP_LIMITS, era: () => era };
}

export async function forgetRecentCopy(): Promise<void> {
  era += 1;
  await desktopRecentCopy()?.store.clear();
}
