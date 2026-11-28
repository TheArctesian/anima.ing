import type { HubResult } from '$lib/hub/hub';
import type { RemoteCodeApi } from '../api';

type Intent = 'launch' | readonly ['prompt' | 'answer' | 'stop', string];

export function recoveryScope(endpoint: string, account: string, api: RemoteCodeApi) {
  let active = true;
  const refused = {
    ok: false as const,
    error: { code: 'FORBIDDEN' as const, message: 'Sign in with the account that requested this action.' },
  };

  function key(intent: Intent): string {
    return `anima.code.receipt:${JSON.stringify([endpoint, account, intent])}`;
  }

  return {
    account,
    get active() { return active; },
    hasLaunch(): boolean {
      try {
        return localStorage.getItem(key('launch')) !== null;
      } catch {
        return false;
      }
    },
    close() { active = false; },
    async authorize(): Promise<HubResult<null>> {
      if (!active) return refused;
      const result = await api.operator();
      if (!active) return refused;
      if (!result.ok) return result;
      if (result.value?.id !== account) {
        active = false;
        return refused;
      }
      return { ok: true, value: null };
    },
    key,
  };
}

export type RecoveryScope = ReturnType<typeof recoveryScope>;
