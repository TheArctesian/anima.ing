export interface HubAddress {
  /** `http://` only for this computer (NET-004). */
  readonly origin: string;
  readonly http: string;
  readonly socket: string;
}

type AddressProblem = 'not-a-url' | 'not-https' | 'has-credentials' | 'has-path';

/** The message never repeats the input, which may hold a password. */
interface AddressError {
  readonly problem: AddressProblem;
  readonly message: string;
}

type AddressResult = { readonly ok: true; readonly value: HubAddress } | { readonly ok: false; readonly error: AddressError };

/** The client has no default hub (NET-005, DNS-001, R-225); `.invalid` never resolves (RFC 2606), so nothing is sent. */
export const NO_HUB_ADDRESS: HubAddress = Object.freeze({
  origin: 'https://no-hub.invalid',
  http: 'https://no-hub.invalid/graphql',
  socket: 'wss://no-hub.invalid/graphql',
});

/** Hosts that may use plain `http` and `ws` (CLI-002, F-1). */
// No `[::1]`: a CSP source cannot name an IPv6 literal. No `localhost`: it may resolve to ::1, where another user can bind the port.
const LOOPBACK_HOSTS: readonly string[] = ['127.0.0.1'];

const MESSAGES: Record<AddressProblem, string> = {
  'not-a-url': 'Type a host name, for example hub.example.net.',
  'not-https': 'The hub address must use https. Only this computer may use http.',
  'has-credentials': 'Remove the user name and password from the hub address.',
  'has-path': 'Type only the host name and port, no path.',
};

function refused(problem: AddressProblem): AddressResult {
  return { ok: false, error: { problem, message: MESSAGES[problem] } };
}

/** A hub at a port of this computer (its link, HMD-037, or a local hub) is not reached over the VPN. */
export function isOnThisComputer(address: Pick<HubAddress, 'origin'>): boolean {
  return URL.canParse(address.origin) && LOOPBACK_HOSTS.includes(new URL(address.origin).hostname);
}

/** A bare host name means `https://` (NET-005); `http://` is only for `127.0.0.1` (CLI-002, F-1). */
export function parseHubAddress(text: string): AddressResult {
  const trimmed = text.trim();
  const withScheme = trimmed.includes('://') ? trimmed : `https://${trimmed}`;
  if (trimmed === '' || !URL.canParse(withScheme)) return refused('not-a-url');
  const url = new URL(withScheme);
  const plain = url.protocol === 'http:' && LOOPBACK_HOSTS.includes(url.hostname);
  if (url.protocol !== 'https:' && !plain) return refused('not-https');
  if (url.username !== '' || url.password !== '') return refused('has-credentials');
  if (url.pathname !== '/' || url.search !== '' || url.hash !== '') return refused('has-path');
  return {
    ok: true,
    value: Object.freeze({ origin: url.origin, http: `${url.origin}/graphql`, socket: `${plain ? 'ws' : 'wss'}://${url.host}/graphql` }),
  };
}
