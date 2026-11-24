import { invoke, isTauri } from '@tauri-apps/api/core';

/** Who listens on a loopback hub address's port (F-10); `own` includes the app's own child on the device link's port. */
export type Listener = 'own' | 'other' | 'none';

export type ListenerProblem = 'caller' | 'bad-origin' | 'unavailable' | 'failed';

export type ListenerResult = { readonly ok: true; readonly listener: Listener } | { readonly ok: false; readonly problem: ListenerProblem };

const LISTENERS: readonly unknown[] = ['own', 'other', 'none'] satisfies Listener[];
const PROBLEMS: readonly unknown[] = ['caller', 'bad-origin'] satisfies ListenerProblem[];

export async function checkHubListener(origin: string): Promise<ListenerResult> {
  if (!isTauri()) return { ok: false, problem: 'unavailable' };
  try {
    const answer: unknown = await invoke('hub_listener_check', { origin });
    return LISTENERS.includes(answer) ? { ok: true, listener: answer as Listener } : { ok: false, problem: 'failed' };
  } catch (reason) {
    const code = typeof reason === 'object' && reason !== null && 'code' in reason ? reason.code : undefined;
    return { ok: false, problem: PROBLEMS.includes(code) ? (code as ListenerProblem) : 'failed' };
  }
}
