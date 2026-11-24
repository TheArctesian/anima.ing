import { invoke, isTauri } from '@tauri-apps/api/core';
import type { HubAddress } from '$lib/hub/hub';

/** The CLI issues and stores a linked credential under its process lock. Native code queues lifecycle changes. */
export async function startCommandLineSignIn(address: HubAddress, appToken: string): Promise<void> {
  if (!isTauri()) return;
  try {
    await invoke('command_line_adopt', { hub: address.origin, token: appToken });
  } catch {
    // Neither the native error nor the request is logged: the request carries the app session.
    console.warn('Command-line sign-in did not complete. Retry by reopening Anima.');
  }
}

/** Finish the CLI handoff before a navigation can replace this web view. */
export function writeTokenAndCommandLine(
  writeToken: (address: HubAddress, token: string) => Promise<void>,
): (address: HubAddress, token: string) => Promise<void> {
  return async (address, token) => {
    await writeToken(address, token);
    await startCommandLineSignIn(address, token);
  };
}

export async function signOutCommandLine(address: HubAddress): Promise<boolean> {
  if (!isTauri()) return true;
  try {
    await invoke('command_line_sign_out', { hub: address.origin });
    return true;
  } catch {
    return false;
  }
}
