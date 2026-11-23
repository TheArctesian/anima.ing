import type { Level, StayAwakeProblem, StayAwakeStatus } from '$lib/desktop/stay-awake/shell';

/** In the order the control shows them (AWK-001). */
export const LEVELS: readonly { readonly value: Level; readonly label: string }[] = [
  { value: 'off', label: 'Off' },
  { value: 'awake', label: 'Keep awake' },
  { value: 'lid-closed', label: 'Keep awake with the lid closed' },
];

/** Percent (AWK-006). */
export const FLOORS: readonly number[] = [10, 20, 30, 40, 50];

const PROBLEMS: Readonly<Record<StayAwakeProblem, string>> = {
  caller: 'Only the main window can change this.',
  unsaved: 'Not saved. Nothing changed.',
  unsupported: 'This computer has no set-up step.',
  refused: 'Administrator step cancelled or unfinished. Nothing changed.',
  failed: 'Could not change stay awake.',
};

/** The shell's own message is never shown. */
export function problemText(problem: StayAwakeProblem): string {
  return PROBLEMS[problem];
}

const HELD: Readonly<Record<Level, string>> = {
  off: 'Sleeps as usual.',
  awake: 'No idle sleep. The display can sleep.',
  'lid-closed': 'Stays on with the lid closed. It gets warm: keep it on a hard surface.',
};

/** Says why when the computer does less than the choice (AWK-006, AWK-008). */
export function describeStayAwake(status: StayAwakeStatus): string {
  switch (status.pause) {
    case 'low-battery':
      return `Paused: battery below ${String(status.batteryFloor ?? 0)}%. Resumes on mains power.`;
    case 'lid-not-ready':
      return status.lid === 'needs-set-up'
        ? 'Lid-closed is not set up. Awake only while the lid is open.'
        : 'This computer refused lid-closed. Awake only while the lid is open.';
    case 'unavailable':
      return 'This system refused stay awake. Sleeps as usual.';
    case null:
      return HELD[status.held];
  }
}

/** A loopback origin does not decide it: a joined device reaches its hub on loopback through the link (AWK-005, FRN-008). */
export function hubIsThisComputer(origin: string, localHub: string | null): boolean {
  return localHub !== null && URL.canParse(origin) && URL.canParse(localHub) && new URL(origin).origin === new URL(localHub).origin;
}

export function sleepNotice(thisComputerIsHub: boolean, status: StayAwakeStatus): string | null {
  return thisComputerIsHub && status.held === 'off' ? 'This computer is the hub: checks stop while it sleeps.' : null;
}

/** A saved floor that is not a fixed step stays a choice (AWK-006). */
export function floorChoices(saved: number | null): readonly number[] {
  return saved === null || FLOORS.includes(saved) ? FLOORS : [...FLOORS, saved].sort((a, b) => a - b);
}
