/** Plain absolute segments only: no `.` or `..`, query, fragment or host; a `//` start never matches (K4 §1). */
const TUNNEL_PATH = /^(?:\/(?!\.{1,2}(?:\/|$))[A-Za-z0-9._~-]+)+$/;

const LOOPBACK = /^(?:localhost|127(?:\.\d{1,3}){3}|\[::1\])$/;

/** `ws` only to a loopback hub: anything else would send the ticket token in clear text (SEC-002). */
function socketScheme(hub: URL): string | undefined {
  if (hub.protocol === 'https:') return 'wss:';
  if (hub.protocol === 'http:' && LOOPBACK.test(hub.hostname)) return 'ws:';
  return undefined;
}

/** The ticket token is not in the URL: the library appends `?token=…` when it connects (D-3). */
export function tunnelUrl(origin: string, tunnelPath: string): string | undefined {
  if (!URL.canParse(origin) || !TUNNEL_PATH.test(tunnelPath)) return undefined;
  const hub = new URL(origin);
  const scheme = socketScheme(hub);
  if (scheme === undefined || hub.username !== '' || hub.password !== '') return undefined;
  return `${scheme}//${hub.host}${tunnelPath}`;
}
