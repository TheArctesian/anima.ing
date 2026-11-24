import { NO_HUB_ADDRESS, parseHubAddress, type HubAddress } from '$lib/hub/hub';

const STORAGE_KEY = 'anima.hubAddress';

/** Storage may refuse to be read in privacy modes. */
function readText(storage: Storage): string | null {
  try {
    return storage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

/** The address of no hub when nothing is stored or the stored text no longer parses (CLI-002, R-225). */
export function storedHubAddress(storage: Storage): HubAddress {
  const text = readText(storage);
  if (text === null) return NO_HUB_ADDRESS;
  const parsed = parseHubAddress(text);
  return parsed.ok ? parsed.value : NO_HUB_ADDRESS;
}

/** Even an address that no longer parses counts: a first-run sign (FRN-001, NAV-009). */
export function hasStoredHubAddress(storage: Storage): boolean {
  return readText(storage) !== null;
}

/** A refusal saves nothing and keeps the old address (CLI-002). */
export function storeHubAddress(storage: Storage, text: string): ReturnType<typeof parseHubAddress> {
  const parsed = parseHubAddress(text);
  if (parsed.ok) storage.setItem(STORAGE_KEY, parsed.value.origin);
  return parsed;
}

/** For when this device leaves the hub of its link (M-C). */
export function forgetHubAddress(storage: Storage, origin: string): void {
  if (readText(storage) === origin) storage.removeItem(STORAGE_KEY);
}
