import { invoke, isTauri } from '@tauri-apps/api/core';
import { parseHubAddress, type HubAddress } from '$lib/hub/hub';

export type AccessTokenProblem = 'unavailable' | 'denied' | 'failed' | 'empty' | 'origin' | 'caller' | 'not-desktop';

const MESSAGES: Record<AccessTokenProblem, string> = {
  unavailable: 'The key store is not available.',
  denied: 'The key store refused access.',
  failed: 'The key store failed.',
  empty: 'The access token is empty.',
  origin: 'The hub address is not valid for the key store.',
  caller: 'Only the main window may use the builder token.',
  'not-desktop': 'The access token is kept by the desktop app only.',
};

const SHELL_CODES: readonly AccessTokenProblem[] = ['unavailable', 'denied', 'failed', 'empty', 'origin', 'caller'];

/** A key-store failure. The message is fixed text chosen here; nothing from the shell's error is kept (LOG-007). */
export class AccessTokenError extends Error {
  constructor(readonly problem: AccessTokenProblem) {
    super(MESSAGES[problem]);
    this.name = 'AccessTokenError';
  }
}

function problemOf(reason: unknown): AccessTokenProblem {
  const code = typeof reason === 'object' && reason !== null && 'code' in reason ? reason.code : undefined;
  return SHELL_CODES.find((known) => known === code) ?? 'failed';
}

async function invokeKeyStore(command: string, args: Record<string, unknown>): Promise<unknown> {
  try {
    return await invoke(command, args);
  } catch (reason) {
    // No `cause`: the shell's error could carry the token (LOG-007).
    throw new AccessTokenError(problemOf(reason));
  }
}

function requireDesktop(): void {
  if (!isTauri()) throw new AccessTokenError('not-desktop');
}

/** A hub's token goes only to that hub's endpoints, so a hand-built address cannot pair one hub's key with another's (CLI-002, CLI-003). */
function keyOf(address: HubAddress): string {
  const parsed = parseHubAddress(address.origin);
  const same =
    parsed.ok && parsed.value.origin === address.origin && parsed.value.http === address.http && parsed.value.socket === address.socket;
  if (!same) throw new AccessTokenError('origin');
  return address.origin;
}

/** `null` off desktop or when none is stored; a key-store failure rejects with `AccessTokenError` (CLI-002). */
export async function readAccessToken(address: HubAddress): Promise<string | null> {
  // In a browser a spoofed `isTauri()` only reaches `failed` (no shell, nothing to leak).
  if (!isTauri()) return null;
  const token = await invokeKeyStore('access_token_read', { origin: keyOf(address) });
  if (token === null) return null;
  if (typeof token !== 'string') throw new AccessTokenError('failed');
  return token;
}

/** Write with the `hub.address` of the hub that ran the login mutation (CLI-003). */
export async function writeAccessToken(address: HubAddress, token: string): Promise<void> {
  requireDesktop();
  await invokeKeyStore('access_token_write', { origin: keyOf(address), token });
}

export async function clearAccessToken(address: HubAddress): Promise<void> {
  requireDesktop();
  await invokeKeyStore('access_token_clear', { origin: keyOf(address) });
}

export async function readBuilderToken(address: HubAddress): Promise<string | null> {
  if (!isTauri()) return null;
  const token = await invokeKeyStore('builder_token_read', { origin: keyOf(address) });
  if (token === null) return null;
  if (typeof token !== 'string') throw new AccessTokenError('failed');
  return token;
}

/** Keep the builder token `enrolBuilder` returned in the OS key store, never in `localStorage` (BLD-023). */
export async function writeBuilderToken(address: HubAddress, token: string): Promise<void> {
  requireDesktop();
  await invokeKeyStore('builder_token_write', { origin: keyOf(address), token });
}

export async function clearBuilderToken(address: HubAddress): Promise<void> {
  requireDesktop();
  await invokeKeyStore('builder_token_clear', { origin: keyOf(address) });
}
