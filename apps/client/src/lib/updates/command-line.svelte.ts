import {
  installCommandLine,
  readCommandLine,
  removeCommandLine,
  type CommandLineResult,
  type CommandLineStatus,
} from '$lib/desktop/command-line/shell';
import { installTarget, refusalText } from './command-line';

export interface CommandLineShell {
  readonly read: () => Promise<CommandLineResult>;
  readonly install: (folder: string) => Promise<CommandLineResult>;
  readonly remove: () => Promise<CommandLineResult>;
}

export const desktopCommandLineShell: CommandLineShell = {
  read: readCommandLine,
  install: installCommandLine,
  remove: removeCommandLine,
};

/** The "Command line tool" row (INS-006): Install runs only after the operator confirms the exact path the shell proposed. */
export class CommandLineTool {
  status = $state<CommandLineStatus | null>(null);
  confirming = $state<{ readonly folder: string; readonly path: string } | null>(null);
  busy = $state(false);
  problem = $state<string | null>(null);

  constructor(private readonly shell: CommandLineShell) {}

  private async run(call: () => Promise<CommandLineResult>, path: string | null): Promise<void> {
    this.busy = true;
    this.problem = null;
    const result = await call();
    this.busy = false;
    if (result.ok) {
      this.status = result.status;
      return;
    }
    this.problem = refusalText(result.problem, result.occupant, path);
    // A refused change may leave the state other than it was shown: read it again.
    if (result.problem !== 'caller') await this.reread();
  }

  private async reread(): Promise<void> {
    const result = await this.shell.read();
    if (result.ok) this.status = result.status;
  }

  load(): Promise<void> {
    return this.run(() => this.shell.read(), null);
  }

  askInstall(): void {
    this.problem = null;
    this.confirming = installTarget(this.status);
  }

  cancel(): void {
    this.confirming = null;
  }

  async confirm(): Promise<void> {
    const target = this.confirming;
    if (target === null || this.busy) return;
    this.confirming = null;
    await this.run(() => this.shell.install(target.folder), target.path);
  }

  async remove(): Promise<void> {
    if (this.busy) return;
    this.confirming = null;
    await this.run(() => this.shell.remove(), null);
  }
}
