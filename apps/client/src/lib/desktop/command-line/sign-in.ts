import { invoke, isTauri } from '@tauri-apps/api/core';
import { accessApi } from '$lib/access/api';
import { createRequestHub, type HubAddress, type HubResult } from '$lib/hub/hub';

/**
 * One sign-in (R-383, SGN-NEW-adopt): when the desktop app signs in to a hub, the `anima` program that it carries is
 * signed in too, with a token that the hub links to the app's token; when the app signs out, so does `anima`. The
 * token goes from the hub's answer straight to the shell, which gives it to `anima` on standard input; nothing here
 * keeps, shows or logs it (CLI-003, LOG-007). A failure never stops the app's own sign-in or sign-out.
 */

/** Why the command line was not signed in or out. */
export type CommandLineSignInProblem = 'not-desktop' | 'refused' | 'failed';

/** What the command-line sign-in needs; each part is injected, so that tests need no shell and no hub. */
export interface CommandLineSignIn {
  readonly computer: () => Promise<string>;
  readonly issue: (address: HubAddress, appToken: string, computer: string) => Promise<HubResult<string>>;
  readonly adopt: (origin: string, token: string) => Promise<void>;
}

/** The name `issueCommandLineToken` gets when the OS gives none. */
export const UNNAMED_COMPUTER = 'This computer';

/**
 * Signs in the `anima` program of this computer to the hub at `address`, with the app's token `appToken`. Returns
 * `null` when it is signed in, or the problem.
 */
export async function signInCommandLine(
  parts: CommandLineSignIn,
  address: HubAddress,
  appToken: string,
): Promise<CommandLineSignInProblem | null> {
  try {
    const name = (await parts.computer()).trim();
    const issued = await parts.issue(address, appToken, name === '' ? UNNAMED_COMPUTER : name.slice(0, 64));
    if (!issued.ok) return 'refused';
    await parts.adopt(address.origin, issued.value);
    return null;
  } catch {
    // The shell or the hub did not answer; the app stays signed in and `anima auth login` still works.
    return 'failed';
  }
}

/** The command-line sign-in of the desktop shell and the hub. */
export function shellCommandLineSignIn(): CommandLineSignIn {
  return {
    computer: async () => {
      const name: unknown = await invoke('command_line_computer');
      return typeof name === 'string' ? name : '';
    },
    issue: (address, appToken, computer) =>
      accessApi(createRequestHub({ address, token: () => Promise.resolve(appToken) })).issueCommandLineToken(computer),
    adopt: async (origin, token) => {
      await invoke('command_line_adopt', { hub: origin, token });
    },
  };
}

/**
 * After the app wrote its token for `address`, signs in `anima` too, without waiting: the app's sign-in does not wait
 * for the command line. Off desktop there is no `anima` to sign in.
 */
export function startCommandLineSignIn(address: HubAddress, appToken: string): void {
  if (!isTauri()) return;
  // The outcome shows nowhere yet: a person who needs `anima` sees it signed out and can run `anima auth login`.
  void signInCommandLine(shellCommandLineSignIn(), address, appToken);
}

/** `writeToken` that, once the app's token is written, also signs in `anima` (R-383). */
export function writeTokenAndCommandLine(
  writeToken: (address: HubAddress, token: string) => Promise<void>,
): (address: HubAddress, token: string) => Promise<void> {
  return async (address, token) => {
    await writeToken(address, token);
    startCommandLineSignIn(address, token);
  };
}

/** Signs `anima` out of the hub at `address`; `false` when it could not. The hub already ended its token (R-383). */
export async function signOutCommandLine(address: HubAddress): Promise<boolean> {
  if (!isTauri()) return true;
  try {
    await invoke('command_line_sign_out', { hub: address.origin });
    return true;
  } catch {
    // The hub ended the token with the app's: what stays is a token that no hub takes.
    return false;
  }
}
