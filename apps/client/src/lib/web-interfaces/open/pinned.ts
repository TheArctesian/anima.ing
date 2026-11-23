import { displayFingerprint, fingerprintFrom } from '$lib/web-interfaces/shell/fingerprint';

/** Only an `https` interface shows a pin, and only a valid SHA-256 fingerprint (WEB-011, D-5). */
export function pinnedFingerprintToShow(webInterface: { readonly url: string; readonly pinnedFingerprintSha256?: string | null }): string | null {
  if (URL.parse(webInterface.url)?.protocol !== 'https:') return null;
  const pin = fingerprintFrom(webInterface.pinnedFingerprintSha256);
  return pin === null ? null : displayFingerprint(pin);
}
