import type { HubAddress, HubError, HubResult, TokenProvider } from '$lib/hub/hub';
import { isSignedOut } from './routes';

/** Injected, so this file stays pure (WP-15 D-4). */
type GoToSignIn = () => void;

interface FirstRunSign {
  readonly addressSaved: boolean;
  readonly goToWelcome: () => void;
}

interface StartCheck {
  readonly route: string;
  readonly address: HubAddress;
  readonly token: TokenProvider;
  readonly desktop: boolean;
  readonly me: () => Promise<HubResult<unknown>>;
  readonly goToSignIn: GoToSignIn;
  readonly firstRun?: FirstRunSign;
}

/** Only a saved address chooses a hub; an older installation's token does not (HMD-022). */
export function isFirstRun(desktop: boolean, addressSaved: boolean): boolean {
  return desktop && !addressSaved;
}

/**
 * A browser cannot read its `HttpOnly` cookie, so it asks the hub `me`: only UNAUTHENTICATED goes to sign-in, and any
 * other failure stays for the hub status to show (API-003, E-3, NET-004).
 */
export async function startGuard(check: StartCheck): Promise<void> {
  if (isSignedOut(check.route)) return;
  if (check.firstRun !== undefined && isFirstRun(check.desktop, check.firstRun.addressSaved)) {
    check.firstRun.goToWelcome();
    return;
  }
  if (!check.desktop) {
    const result = await check.me();
    if (!result.ok) signInNeeded(result.error, check.goToSignIn);
    return;
  }
  const read = await check.token(check.address).then(
    (token) => ({ ok: true as const, token }),
    () => ({ ok: false as const }),
  );
  if (read.ok && read.token !== null && read.token !== '') return;
  check.goToSignIn();
}

export function signInNeeded(error: HubError, goToSignIn: GoToSignIn): boolean {
  if (error.code !== 'UNAUTHENTICATED') return false;
  goToSignIn();
  return true;
}
