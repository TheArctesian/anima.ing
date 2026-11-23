import type { SessionTransfer, SessionsApi } from './api';

/** The copy and paste flags one session obeys (RA-033). */
export interface SessionPolicy extends SessionTransfer {
  /** False when the hub did not answer. */
  readonly read: boolean;
}

export const UNREAD_POLICY: SessionPolicy = { copy: false, paste: false, read: false };

/** Every failure gives `UNREAD_POLICY`, and a flag that is not exactly `true` is off (fail closed). */
export async function readPolicy(api: Pick<SessionsApi, 'transferPolicy'>, deviceId: string): Promise<SessionPolicy> {
  try {
    const result = await api.transferPolicy(deviceId);
    return result.ok ? { copy: result.value.copy === true, paste: result.value.paste === true, read: true } : UNREAD_POLICY;
  } catch {
    return UNREAD_POLICY;
  }
}
