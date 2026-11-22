/**
 * What the person typed in the address bar of the browser surface, as an address to open (task TC5; report 05,
 * Section 6.2). Only http and https open. Text that is not an address is refused with words: the surface does not send
 * it to a search service.
 */
export type Address = { readonly ok: true; readonly url: string } | { readonly ok: false; readonly title: string; readonly detail: string };

const REFUSED: Address = { ok: false, title: 'Could not open this address', detail: 'Enter an http or https URL.' };
const SCHEME = /^[a-z][a-z0-9+.-]*:\/\//i;
/** A host of this computer or a bare IPv4 address: these are opened with http, as a dev server answers. */
const LOCAL = /^(localhost|127(\.\d{1,3}){3}|0\.0\.0\.0|\[::1\]|(\d{1,3}\.){3}\d{1,3})(:\d+)?(\/|$)/i;

function parsed(text: string): URL | undefined {
  try {
    return new URL(text);
  } catch {
    return undefined;
  }
}

export function addressOf(typed: string): Address {
  const text = typed.trim();
  if (text === '' || /\s/.test(text)) return REFUSED;
  const withScheme = SCHEME.test(text) ? text : `${LOCAL.test(text) ? 'http' : 'https'}://${text}`;
  const url = parsed(withScheme);
  if (url === undefined || (url.protocol !== 'http:' && url.protocol !== 'https:')) return REFUSED;
  // A word with no dot and no port is not a host that the person meant: "readme" is not an address.
  if (!SCHEME.test(text) && !LOCAL.test(text) && !url.hostname.includes('.')) return REFUSED;
  return { ok: true, url: url.href };
}
