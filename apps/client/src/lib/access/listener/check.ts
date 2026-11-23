import { readAccessToken } from '$lib/desktop/token';
import { checkHubListener } from '$lib/desktop/hub-listener/shell';
import { readLink } from '$lib/desktop/link/shell';
import type { HubAddress, TokenProvider } from '$lib/hub/hub';
import { listenerGate, listenerProblem, needsListenerCheck, type ListenerProblem, type TokenRead } from './listener';

// One gate for the page, shared by every token reader and the sign-in page (F-10).

/** Each answer stands for a few seconds. */
export const hubListeners = listenerGate({ check: checkHubListener, now: () => Date.now() });

/** Behind the listener check when the address is this computer's (F-10, S-1). */
export function gatedReader(reader: TokenProvider, address: HubAddress, desktop: boolean, read?: TokenRead): TokenProvider {
  return needsListenerCheck(address.origin, desktop) ? hubListeners.token(reader, read) : reader;
}

export function tokenFor(address: HubAddress, desktop: boolean, read?: TokenRead): TokenProvider {
  return gatedReader(readAccessToken, address, desktop, read);
}

export function forgetListener(origin: string): void {
  hubListeners.forget(origin);
}

/** Always a fresh answer: it is asked right before a password would be sent (F-10). */
export async function loopbackProblem(origin: string, desktop: boolean): Promise<ListenerProblem | null> {
  if (!needsListenerCheck(origin, desktop)) return null;
  hubListeners.forget(origin);
  const verdict = await hubListeners.verdict(origin);
  if (verdict.safe) return null;
  const link = await readLink();
  return listenerProblem(verdict, origin, link.ok ? link.value : null);
}
