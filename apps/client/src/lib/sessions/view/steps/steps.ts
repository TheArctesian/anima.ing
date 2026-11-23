export interface Progress {
  readonly ticketed: boolean;
  /** `SessionTicket.wakeSent` (K4 §1); false until the hub answers. */
  readonly waking: boolean;
  readonly opened: boolean;
  readonly connected: boolean;
  readonly stopped: boolean;
}

export type StepName = 'request' | 'gateway' | 'device' | 'ready';

export type StepMark = 'waiting' | 'active' | 'done' | 'failed';

export interface Step {
  readonly name: StepName;
  readonly label: string;
  readonly mark: StepMark;
}

/**
 * Section 5.7.3. The hub upgrades the WebSocket only after it signed in to the device (K4 §8.2); no signal follows
 * `connected`, so `ready` ends with `device`.
 */
const STEPS: readonly { readonly name: StepName; readonly label: string; readonly ended: (p: Progress) => boolean }[] = [
  { name: 'request', label: 'Request', ended: (p) => p.ticketed },
  { name: 'gateway', label: 'Gateway', ended: (p) => p.opened },
  { name: 'device', label: 'Device', ended: (p) => p.connected },
  { name: 'ready', label: 'Ready', ended: (p) => p.connected },
];

export function connectionSteps(progress: Progress): readonly Step[] {
  const current = STEPS.findIndex((step) => !step.ended(progress));
  return STEPS.map(({ name, label }, index) => {
    if (current === -1 || index < current) return { name, label, mark: 'done' };
    if (index > current) return { name, label, mark: 'waiting' };
    return { name, label, mark: progress.stopped ? 'failed' : 'active' };
  });
}

export interface WakeLine {
  readonly label: string;
  readonly mark: StepMark;
}

/** The hub opens the socket once the device answers, so the wait ends with `opened`. */
export function wakeLine(progress: Progress, deviceName: string): WakeLine | undefined {
  if (!progress.waking) return undefined;
  const mark: StepMark = progress.opened ? 'done' : progress.stopped ? 'failed' : 'active';
  return { label: `Waking ${deviceName}`, mark };
}
