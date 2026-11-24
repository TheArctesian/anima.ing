import type { HubError } from '../error';

// The hosted hubs' gateway keeps the paths below `/_door/` and refuses with `{ error: { code, message } }` (HMD-059, HMD-061).

/** The gateway makes the session of the hub again, or starts a sign-in. */
export const GATEWAY_ENTER_PATH = '/_door/enter';
/** A `POST` from a page of the gateway's own origin. */
export const SIGN_OUT_PATH = '/_door/sign-out';
/** No route of the gateway: only a gateway answers it with the code `no_such_route`. */
export const PROBE_PATH = '/_door/client';

const CODES = ['sign_in_needed', 'hub_not_ready', 'hub_failed', 'too_many_requests', 'no_such_route'] as const;

export type GatewayCode = (typeof CODES)[number];

function isObject(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function gatewayCode(body: unknown): GatewayCode | null {
  if (!isObject(body) || !isObject(body.error)) return null;
  const code = body.error.code;
  return CODES.find((known) => known === code) ?? null;
}

const MESSAGES: Record<GatewayCode, HubError> = {
  // The same code as a hub that refuses the cookie, so each page sends the browser to the sign-in (API-003).
  sign_in_needed: { code: 'UNAUTHENTICATED', message: 'This browser is signed out. Sign in again.' },
  hub_not_ready: { code: 'UNAVAILABLE', message: 'Your hub is starting. Try again in a few seconds.' },
  hub_failed: { code: 'UNAVAILABLE', message: 'Your hub did not answer. Try again.' },
  too_many_requests: { code: 'UNAVAILABLE', message: 'Too many requests. Wait a minute.' },
  no_such_route: { code: 'malformed', message: "Can't read the hub's answer." },
};

export function gatewayError(code: GatewayCode): HubError {
  return MESSAGES[code];
}

/** The hub then counts as not reached (NET-004). */
export function hubDidNotAnswer(code: GatewayCode): boolean {
  return code === 'hub_not_ready' || code === 'hub_failed';
}

export function isGatewayAnswer(status: number, body: unknown): boolean {
  return status === 404 && gatewayCode(body) === 'no_such_route';
}

/** A hub that takes no session does not loop the browser through the entry. */
export const ENTER_AGAIN_MS = 60_000;

export function mayEnterGatewayNow(lastMs: number | null, nowMs: number): boolean {
  return lastMs === null || nowMs - lastMs >= ENTER_AGAIN_MS || nowMs < lastMs;
}

/** The redirect counts as success; the page does not follow it, since `form-action 'none'` allows no form. */
export async function leaveGateway(send: typeof fetch, origin: string): Promise<boolean> {
  try {
    const answer = await send(`${origin}${SIGN_OUT_PATH}`, { method: 'POST', redirect: 'manual', credentials: 'same-origin' });
    return answer.type === 'opaqueredirect' || answer.ok || answer.status === 303;
  } catch {
    // No answer: the session of the gateway may still be open.
    return false;
  }
}

export async function askGateway(send: typeof fetch, origin: string): Promise<boolean> {
  try {
    const answer = await send(`${origin}${PROBE_PATH}`, { headers: { accept: 'application/json' }, credentials: 'same-origin' });
    const body: unknown = await answer.json();
    return isGatewayAnswer(answer.status, body);
  } catch {
    // No answer, or a page in place of JSON: a hub alone.
    return false;
  }
}

export function gatewayInFront(origin: string): Promise<boolean> {
  return askGateway(fetch, origin);
}

export function gatewaySignOut(origin: string): Promise<boolean> {
  return leaveGateway(fetch, origin);
}
