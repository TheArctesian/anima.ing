import type { ThisComputerShell } from '$lib/desktop/daemon/shell';
import { isThisComputer } from '$lib/devices/screen-sharing/setup';
import type { DaemonCalls } from './install.svelte';

export interface PanelDevice {
  readonly id: string;
  readonly host: string;
}

/** The enrolment code goes from the hub straight to the shell; it is never kept in state, shown or copied (MET-019, LOG-007). */
export class ThisComputerInstall {
  offered = $state(false);
  busy = $state(false);
  error = $state<string | null>(null);
  installed = $state(false);

  /** Counts disposals, so an answer that arrives after the panel closed changes nothing. */
  private epoch = 0;

  constructor(
    private readonly calls: Pick<DaemonCalls, 'enrolDaemon'>,
    private readonly shell: ThisComputerShell | null,
    private readonly device: PanelDevice,
    private readonly origin: string,
  ) {}

  async check(): Promise<void> {
    if (this.shell === null) return;
    const epoch = this.epoch;
    const machine = await this.shell.thisMachine();
    if (epoch !== this.epoch || !isThisComputer(this.device, machine)) return;
    const bundled = await this.shell.bundled();
    if (epoch === this.epoch) this.offered = bundled;
  }

  async install(): Promise<boolean> {
    if (this.shell === null || !this.offered || this.busy) return false;
    const epoch = this.epoch;
    this.busy = true;
    this.error = null;
    this.installed = false;
    const error = await this.installWithFreshCode(this.shell, epoch);
    if (epoch !== this.epoch) return false;
    this.busy = false;
    this.error = error;
    this.installed = error === null;
    return this.installed;
  }

  /** The only place the code is held: a local of this call, gone when it returns. */
  private async installWithFreshCode(shell: ThisComputerShell, epoch: number): Promise<string | null> {
    const made = await this.calls.enrolDaemon(this.device.id);
    if (epoch !== this.epoch) return null;
    if (!made.ok) return made.error.message;
    const done = await shell.install(made.value.code, this.origin);
    return done.ok ? null : done.message;
  }

  dispose(): void {
    this.epoch += 1;
  }
}
