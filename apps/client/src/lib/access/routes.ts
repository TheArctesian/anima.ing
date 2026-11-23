export const SIGN_IN_PATH = '/sign-in';

/** The sign-in of a hosted hub in a browser: the way to the gateway and the account, with no password (HMD-059). */
export const GATEWAY_SIGN_IN_PATH = '/hub-sign-in';

/** The first-run choices of the desktop app (FRN-001, NAV-009). */
export const WELCOME_PATH = '/welcome';

/** The sign-in with the Anima.ing account (HMD-022). */
export const HOSTED_PATH = '/welcome/hosted';

/** A hosted hub signs in with the account, never with the hub password page (R-129, HMD-041, HMD-043). */
export function signInPathFor(hosted: boolean): typeof SIGN_IN_PATH | typeof HOSTED_PATH {
  return hosted ? HOSTED_PATH : SIGN_IN_PATH;
}

function within(route: string, parent: string): boolean {
  return route === parent || route.startsWith(`${parent}/`);
}

/** Decodes as SvelteKit does (`%25` and reserved escapes stay), so the sign-in check sees the path the router matches. */
export function routePath(pathname: string, base: string): string {
  const below = base !== '' && within(pathname, base) ? pathname.slice(base.length) : pathname;
  try {
    return below.split('%25').map(decodeURI).join('%25') || '/';
  } catch {
    // A malformed escape: the router refuses it too, so the undecoded text decides.
    return below || '/';
  }
}

/** Only the sign-in and first-run pages need no sign-in; every other route, `/shared` included, does (API-003, FRN-001, R-111). */
export function isSignedOut(route: string): boolean {
  return within(route, SIGN_IN_PATH) || within(route, WELCOME_PATH) || within(route, GATEWAY_SIGN_IN_PATH);
}
