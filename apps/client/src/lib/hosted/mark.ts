const STORAGE_KEY = 'anima.hostedHub';

/** A marked hub signs in only with the account, never the hub password page; the mark is not a secret (R-129, HMD-037). */
export function markHosted(storage: Storage, origin: string): void {
  storage.setItem(STORAGE_KEY, origin);
}

/** `false` when the storage refuses to be read (privacy modes). */
export function isHostedHub(storage: Storage, origin: string): boolean {
  try {
    return storage.getItem(STORAGE_KEY) === origin;
  } catch {
    return false;
  }
}

function browserStore(): Storage | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage;
  } catch {
    return null;
  }
}

/** The one place the client decides whether a hub is hosted; a hosted hub has no VPN (R-222, DEF-005). */
export function hubIsHosted(origin: string): boolean {
  const storage = browserStore();
  return storage !== null && isHostedHub(storage, origin);
}

/** An older application may hold this address as its own hub (HMD-059, HM-T31, R-269). */
export const HOSTED_BROWSER_ORIGIN = 'https://hub.anima.ing';

/** HMD-062. */
export const ACCOUNT_PAGE = 'https://cloud.anima.ing/account';

export const HOSTED_ADDRESS_MOVED =
  'hub.anima.ing is now the address of the hosted Anima.ing hubs in a browser. Sign in with your Anima.ing account, or give the address of your own hub.';

/** The application reaches a hosted hub through its link, never at this address, so asking there means an old setting. */
export function holdsMovedAddress(pageOrigin: string | null, origin: string): boolean {
  return origin === HOSTED_BROWSER_ORIGIN && pageOrigin !== origin;
}

export function holdsMovedAddressHere(origin: string): boolean {
  return typeof location !== 'undefined' && holdsMovedAddress(location.origin, origin);
}

/** What the operator can check when the hub does not answer, with no full stop (NET-004, R-222). */
export function networkAdvice(hosted: boolean): string {
  return hosted ? "Check this computer's network" : 'Check the VPN client';
}
