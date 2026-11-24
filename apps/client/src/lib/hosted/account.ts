import type { HubAccess } from './access';

/** The implementation keeps the WorkOS tokens itself; only a sealed `HubAccess` leaves it (HMD-022, HMD-034, LOG-007). */
export interface HostedAccount {
  /** `signed-in` when the account's session still holds and no browser step is needed (HMD-043). */
  readonly start: () => Promise<Answer<SignInCode | 'signed-in'>>;
  readonly openPage: (page: string) => Promise<Answer<void>>;
  readonly poll: (code: SignInCode) => Promise<Answer<SignInPoll>>;
  /** The cloud starts the make or wake job when the hub is not ready. */
  readonly hub: () => Promise<Answer<HubState>>;
  /** The link join and this device's credential; the person never sees it (Section 4.3). */
  readonly access: () => Promise<Answer<HubAccess>>;
}

export interface SignInCode {
  readonly code: string;
  readonly page: string;
  /** Seconds between two polls. */
  readonly interval: number;
}

export type SignInPoll = 'waiting' | 'signed-in';

/** `making` builds a new hub and `waking` wakes a sleeping one (HMD-025). */
export type HubState = 'ready' | 'making' | 'waking';

/** `unconfigured`: this build has no account sign-in. `payment`: the account's payment failed, so the cloud makes and wakes nothing. */
export type Problem = 'cancelled' | 'expired' | 'offline' | 'refused' | 'signed-out' | 'unconfigured' | 'payment';

export type Answer<T> = { readonly ok: true; readonly value: T } | { readonly ok: false; readonly problem: Problem };

export function answer<T>(value: T): Answer<T> {
  return { ok: true, value };
}

export function refusal<T>(problem: Problem): Answer<T> {
  return { ok: false, problem };
}
