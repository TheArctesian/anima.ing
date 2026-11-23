import type { FailureReason, UpdateSnapshot } from './schedule';

const NEXT = (nextCheckAt: number | null, time: (at: Date) => string): string =>
  nextCheckAt === null ? '' : ` Next check at ${time(new Date(nextCheckAt))}.`;

function describeFailure(reason: FailureReason): string {
  switch (reason) {
    case 'no-feed':
      return 'No update published for this computer yet.';
    case 'signature':
      return 'Update refused: its signature did not verify. Current version kept.';
    case 'other':
      return 'Last update failed.';
  }
}

/** The update state for the Settings row (UPD-006). */
export function describeUpdate({ state, nextCheckAt }: UpdateSnapshot, time: (at: Date) => string): string {
  switch (state.kind) {
    case 'unchecked':
      return 'Not checked yet';
    case 'idle':
      return 'Up to date';
    case 'checking':
      return 'Checking…';
    case 'downloading':
      return `Downloading Anima.ing ${state.version}…`;
    case 'installed':
      return `Anima.ing ${state.version} is installed — restart to update`;
    case 'failed':
      return describeFailure(state.reason) + NEXT(nextCheckAt, time);
  }
}

export function canCheck({ state }: UpdateSnapshot): boolean {
  return state.kind === 'unchecked' || state.kind === 'idle' || state.kind === 'failed';
}
