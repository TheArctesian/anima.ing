import { goto } from '$app/navigation';
import { resolve } from '$app/paths';
import { hasStoredHubAddress, storedHubAddress } from '$lib/desktop/address';
import { behindGateway } from '$lib/hosted/gateway/here';
import { holdsMovedAddressHere, hubIsHosted } from '$lib/hosted/mark';
import type { Hub } from '$lib/hub/hub';
import { capabilities } from '$lib/platform/platform';
import { accessApi } from './api';
import { startGuard } from './guard';
import { tokenFor } from './listener/check';
import { GATEWAY_SIGN_IN_PATH, isSignedOut, routePath, signInPathFor, SIGN_IN_PATH, WELCOME_PATH } from './routes';
import { clientStart, type ClientStart } from './start';

// The browser side of access; logic stays in `guard.ts` and `start.ts`.

function failed(): void {
  console.warn('Navigation failed.');
}

export function goToSignIn(): void {
  // The desktop app with no saved address has no hub to sign in to: it shows the first-run choices (R-225).
  if (capabilities().keyStore && !hasStoredHubAddress(localStorage)) return goToWelcome();
  // The address of before is the hosted service now: the first-run choices lead to the account or to a hub address (HM-T31).
  if (capabilities().keyStore && holdsMovedAddressHere(storedHubAddress(localStorage).origin)) return goToWelcome();
  // A browser signs in to the hub that served it, never through the desktop app's account sign-in (HMD-059).
  if (!capabilities().keyStore) {
    void behindGateway()
      .then((behind) => goto(resolve(behind ? GATEWAY_SIGN_IN_PATH : SIGN_IN_PATH)))
      .catch(failed);
    return;
  }
  const hosted = hubIsHosted(storedHubAddress(localStorage).origin);
  goto(resolve(signInPathFor(hosted))).catch(failed);
}

export function goToWelcome(): void {
  goto(resolve(WELCOME_PATH)).catch(failed);
}

export function goToDevices(): void {
  goto(resolve('/devices')).catch(failed);
}

/** `location.replace` would not reload an address that differs only by its fragment (WP-16 H-1). */
export function reloadClient(): void {
  location.reload();
}

/** A new page, not a router move: only a new page opens its socket with the cookie as it is now (E-4). */
export function loadClientAt(path: '/devices' | typeof SIGN_IN_PATH | typeof WELCOME_PATH): void {
  location.replace(resolve(path));
}

/** `resolve('/')` is the base path and a slash; SvelteKit's `base` is deprecated. */
function routeOf(url: URL): string {
  return routePath(url.pathname, resolve('/').slice(0, -1));
}

export function showsSignedOutPage(url: URL): boolean {
  return isSignedOut(routeOf(url));
}

export function startAt(url: URL): ClientStart {
  return clientStart({ desktop: capabilities().keyStore, storage: localStorage, origin: url.origin });
}

export function guardStart(url: URL, hub: Hub): void {
  const { me } = accessApi(hub);
  const desktop = capabilities().keyStore;
  const check = { route: routeOf(url), address: hub.address, token: tokenFor(hub.address, desktop), desktop, me };
  const firstRun = { addressSaved: hasStoredHubAddress(localStorage), goToWelcome };
  startGuard({ ...check, goToSignIn, firstRun }).catch(failed);
}
