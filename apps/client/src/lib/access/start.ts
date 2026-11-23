import { storedHubAddress } from '$lib/desktop/address';
import type { HubAddress } from '$lib/hub/hub';
import { servedHubAddress } from '$lib/platform/browser';

/** `refused` sends nothing anywhere: the page's origin cannot be a hub (E-2). */
export type ClientStart =
  | { readonly kind: 'refused'; readonly message: string }
  | { readonly kind: 'shell'; readonly address: HubAddress };

/** What the start decision reads; the caller reads the platform and the browser (CQ-021). */
interface StartPlace {
  readonly desktop: boolean;
  readonly storage: Storage;
  readonly origin: string;
}

/** Fixed text: it never repeats the origin. */
export const ORIGIN_REFUSED = "Anima.ing runs only from its hub's https address. Open that address.";

/** A browser talks only to the hub that served the page and ignores any stored address, so page script cannot redirect it (CLI-002, E-2). */
export function clientStart(place: StartPlace): ClientStart {
  if (place.desktop) return { kind: 'shell', address: storedHubAddress(place.storage) };
  const served = servedHubAddress(place.origin);
  return served === null ? { kind: 'refused', message: ORIGIN_REFUSED } : { kind: 'shell', address: served };
}
