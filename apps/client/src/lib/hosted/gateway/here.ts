import { capabilities } from '$lib/platform/platform';
import { GATEWAY_ENTER_PATH, gatewayInFront, gatewaySignOut, mayEnterGatewayNow } from '$lib/hub/hub';
import { markHosted } from '../mark';

const ENTERED_KEY = 'anima.gatewayEntered';
const LEFT_KEY = 'anima.gatewayLeft';

let asked: Promise<boolean> | undefined;

function session(): Storage | null {
  try {
    return sessionStorage;
  } catch {
    // A privacy mode that refuses storage.
    return null;
  }
}

/** Whether this page came through the hosted hubs' gateway (HMD-059); a hub behind it is marked hosted (R-222, HMD-038). */
export function behindGateway(): Promise<boolean> {
  if (capabilities().keyStore) return Promise.resolve(false);
  asked ??= gatewayInFront(location.origin).then((behind) => {
    if (behind) {
      try {
        markHosted(localStorage, location.origin);
      } catch {
        // The mark only changes words; without storage the page still works.
      }
    }
    return behind;
  });
  return asked;
}

export function leftGateway(): boolean {
  return session()?.getItem(LEFT_KEY) === '1';
}

/** Whether the browser may go to the entry by itself: not after a sign-out, and not right after an entry (no loop). */
export function mayEnterByItself(now: number = Date.now()): boolean {
  if (leftGateway()) return false;
  const last = Number(session()?.getItem(ENTERED_KEY) ?? Number.NaN);
  return mayEnterGatewayNow(Number.isFinite(last) ? last : null, now);
}

export function enterGateway(now: number = Date.now()): void {
  const store = session();
  store?.removeItem(LEFT_KEY);
  store?.setItem(ENTERED_KEY, String(now));
  location.assign(GATEWAY_ENTER_PATH);
}

/** Ends the gateway's session after the hub ended its own (HMD-062); `kept`: the gateway did not confirm. */
export async function signOutOfGateway(): Promise<'none' | 'left' | 'kept'> {
  if (!(await behindGateway())) return 'none';
  if (!(await gatewaySignOut(location.origin))) return 'kept';
  session()?.setItem(LEFT_KEY, '1');
  return 'left';
}
