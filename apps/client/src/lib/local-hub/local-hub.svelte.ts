import { forgetListener } from '$lib/access/listener/check';
import { readLocalHub, startLocalHub, stopLocalHub, type LocalHubResult, type LocalHubStatus } from '$lib/desktop/local-hub/shell';
import { localHubOrigin } from './address';
import { pollUntilSettled } from './state';

export interface LocalHubShell {
  readonly read: () => Promise<LocalHubResult<LocalHubStatus>>;
  readonly start: () => Promise<LocalHubResult<null>>;
  readonly stop: () => Promise<LocalHubResult<null>>;
  readonly wait: (ms: number) => Promise<void>;
  /** Drops the listener answer for an origin, so that the next request checks the port again (F-10, M-C). */
  readonly forget: (origin: string) => void;
}

export const desktopLocalHubShell: LocalHubShell = {
  read: readLocalHub,
  start: startLocalHub,
  stop: stopLocalHub,
  wait: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
  forget: forgetListener,
};

/** The hub on this computer as Settings shows it (FRN-006). */
export class LocalHubWatch {
  status = $state<LocalHubStatus | null>(null);
  busy = $state(false);
  problem = $state<string | null>(null);
  private ended = false;

  constructor(private readonly shell: LocalHubShell) {}

  get origin(): string | null {
    return this.status === null ? null : localHubOrigin(this.status);
  }

  watch(): () => void {
    void this.read();
    return () => {
      this.ended = true;
    };
  }

  private async read(): Promise<void> {
    const result = await this.shell.read();
    if (this.ended) return;
    if (result.ok) this.status = result.value;
    else this.problem = result.message;
  }

  private async change(call: () => Promise<LocalHubResult<null>>, waitForStop: boolean): Promise<void> {
    if (this.busy) return;
    this.busy = true;
    this.problem = null;
    // Another program may take the port while the hub is down, so the answer is forgotten before and after the call (F-10, M-C, MF-1).
    const origin = this.origin;
    if (origin !== null) this.shell.forget(origin);
    const done = await call();
    if (origin !== null) this.shell.forget(origin);
    if (!done.ok) this.problem = done.message;
    if (waitForStop) await this.read();
    else if (done.ok) {
      const last = await pollUntilSettled({ read: this.shell.read, wait: this.shell.wait, seen: (status) => (this.status = status), stopped: () => this.ended });
      if (last !== null && !last.ok) this.problem = last.message;
    }
    // An answer taken while the hub was down ("none") must not outlast the start.
    const after = this.origin;
    if (after !== null) this.shell.forget(after);
    this.busy = false;
  }

  start(): Promise<void> {
    return this.change(this.shell.start, false);
  }

  stop(): Promise<void> {
    return this.change(this.shell.stop, true);
  }
}
