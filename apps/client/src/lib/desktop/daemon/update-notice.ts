import { invoke, isTauri } from '@tauri-apps/api/core';
import { describeDaemonProblem, type DaemonShellProblem } from './shell';

/** What the shell says of the update of the daemon at the start of the app (HMD-055). Versions are three numbers. */
export type DaemonNotice =
  | { readonly kind: 'updated'; readonly from: string | null; readonly to: string }
  | { readonly kind: 'not_updated'; readonly from: string | null; readonly to: string; readonly step: string }
  | { readonly kind: 'older'; readonly installed: string | null; readonly app: string; readonly why: string };

const AGAIN = 'Quit Anima and open it again to update it, or install it again in the settings of this computer, at Metrics source: Daemon.';

function stepWords(step: string): string {
  return describeDaemonProblem(step as DaemonShellProblem).replace(/\.$/, '');
}

/** An older daemon after a failed update is said once; null when there is nothing to say. */
export function noticeWords(notices: readonly DaemonNotice[]): string | null {
  const failed = notices.some((notice) => notice.kind === 'not_updated');
  const words = notices.flatMap((notice): string[] => {
    switch (notice.kind) {
      case 'updated':
        return [`The daemon of this computer was updated${notice.from === null ? '' : ` from ${notice.from}`} to ${notice.to}. If macOS asks whether anima-daemon may use the Keychain, allow it.`];
      case 'not_updated':
        return [`The daemon of this computer was not updated to ${notice.to}: ${stepWords(notice.step)}. The daemon of before${notice.from === null ? '' : ` (${notice.from})`} runs again. ${AGAIN}`];
      case 'older': {
        if (failed) return [];
        const installed = notice.installed === null ? '' : ` (${notice.installed})`;
        const fix = notice.why === 'not_sealed' ? 'The app did not update it: the signature of this app does not verify. Install the app again.' : AGAIN;
        return [`The daemon of this computer${installed} is older than this app (${notice.app}). ${fix}`];
      }
    }
  });
  return words.length === 0 ? null : words.join(' ');
}

function isNotice(value: unknown): value is DaemonNotice {
  return typeof value === 'object' && value !== null && 'kind' in value && ['updated', 'not_updated', 'older'].includes(String(value.kind));
}

/** Once for each update; null outside the app or when the shell does not answer. */
export async function daemonUpdateWords(): Promise<string | null> {
  if (!isTauri()) return null;
  try {
    const read: unknown = await invoke('daemon_update_notice');
    return Array.isArray(read) ? noticeWords(read.filter(isNotice)) : null;
  } catch {
    return null;
  }
}
