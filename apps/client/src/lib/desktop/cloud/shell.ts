import { invoke, isTauri } from '@tauri-apps/api/core';
import { sealAccess } from '$lib/hosted/access';
import { answer, refusal, type Answer, type HostedAccount, type HubState, type Problem, type SignInCode } from '$lib/hosted/account';

/** Mirrors the codes of `apps/desktop/src-tauri/src/cloud/error.rs`. */
const PROBLEMS: Readonly<Record<string, Problem>> = {
  unconfigured: 'unconfigured',
  cancelled: 'cancelled',
  expired: 'expired',
  offline: 'offline',
  'not-ready': 'offline',
  'payment-failed': 'payment',
  unauthenticated: 'signed-out',
};

function problemOf(reason: unknown): Problem {
  const code = typeof reason === 'object' && reason !== null && 'code' in reason ? reason.code : undefined;
  return (typeof code === 'string' ? PROBLEMS[code] : undefined) ?? 'refused';
}

/** A failure is a problem, never its text (LOG-007). */
async function command<T>(name: string, shape: (value: unknown) => T | undefined, args?: Record<string, unknown>): Promise<Answer<T>> {
  if (!isTauri()) return refusal('unconfigured');
  try {
    const value = shape(await invoke(name, args));
    return value === undefined ? refusal('refused') : answer(value);
  } catch (reason) {
    return refusal(problemOf(reason));
  }
}

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null;

function signInCodeOf(value: unknown): SignInCode | undefined {
  if (!isRecord(value) || typeof value.code !== 'string' || typeof value.page !== 'string' || typeof value.interval !== 'number') return undefined;
  return { code: value.code, page: value.page, interval: value.interval };
}

const HUB_STATES: readonly unknown[] = ['ready', 'making', 'waking'] satisfies HubState[];

/** Unchecked; `hosted/usage` checks the form (HMD-073). */
export function cloudHubUsage(): Promise<Answer<unknown>> {
  return command('cloud_hub_usage', (value) => (value === undefined ? undefined : value));
}

export async function clearCloudSession(): Promise<void> {
  await invoke('cloud_sign_out');
}

/**
 * The shell does the calls, so the WorkOS tokens stay in the shell and its key store (HMD-043, HMD-034).
 * With `inApp` (mobile plan M3-5) the shell shows the sign-in page inside the app, so a new sign-in has no code.
 */
export function cloudAccount(inApp = false): HostedAccount {
  return {
    start: async () => {
      // A session that still holds needs no browser step: the account signs in again silently.
      const resumed = await command('cloud_sign_in_resume', (value) => (typeof value === 'boolean' ? value : undefined));
      if (resumed.ok && resumed.value) return answer('signed-in');
      if (!resumed.ok && resumed.problem === 'unconfigured') return resumed;
      if (!inApp) return command('cloud_sign_in_start', signInCodeOf);
      const signedIn = await command('cloud_sign_in_session', (value) => (value === true ? value : undefined));
      return signedIn.ok ? answer('signed-in') : signedIn;
    },
    openPage: async (page) => {
      try {
        await invoke('system_browser_open', { url: page });
        return answer(undefined);
      } catch {
        return refusal('refused');
      }
    },
    poll: async () => {
      const polled = await command('cloud_sign_in_poll', (value) => (typeof value === 'boolean' ? value : undefined));
      return polled.ok ? answer(polled.value ? 'signed-in' : 'waiting') : polled;
    },
    hub: () => command('cloud_hub', (value) => (HUB_STATES.includes(value) ? (value as HubState) : undefined)),
    access: () =>
      command('cloud_hub_access', (value) =>
        isRecord(value) && (typeof value.invite === 'string' || value.invite === null) && typeof value.claim === 'string'
          ? sealAccess({ invite: value.invite, claim: value.claim })
          : undefined,
      ),
  };
}
