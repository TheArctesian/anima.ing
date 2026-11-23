import { applyChange, controlOf, describeChange, nameChange, type McpChange, type McpTool } from './permissions';
import { McpShellError, type McpShell } from '$lib/desktop/mcp/shell';

export type McpStatus = 'loading' | 'ready' | 'failed';

export interface WriteProblem {
  readonly control: string;
  readonly attempt: string;
  readonly reason: string;
}

function messageOf(reason: unknown): string {
  return reason instanceof McpShellError ? reason.message : 'The permissions could not be changed.';
}

/**
 * `tools` lays each change still on its way over what the shell last confirmed; a refused change drops out (MCP-021).
 * Changes reach the shell one at a time, so two never read and write the file together.
 */
export class McpPermissionsStore {
  status = $state<McpStatus>('loading');
  loadProblem = $state('');
  /** The last change that failed. It stays until the operator dismisses it or the same control is saved (MCP-021). */
  problem = $state.raw<WriteProblem | null>(null);
  /** What the last saved change did; one quiet line that a screen reader announces (MCP-022). */
  saved = $state('');
  #confirmed = $state.raw<readonly McpTool[]>([]);
  #queue = $state.raw<readonly McpChange[]>([]);
  #tail: Promise<void> = Promise.resolve();
  readonly tools = $derived(this.#queue.reduce(applyChange, this.#confirmed));

  constructor(
    private readonly shell: McpShell,
    private readonly hub: string,
  ) {}

  async load(): Promise<void> {
    this.status = 'loading';
    try {
      this.#confirmed = (await this.shell.read(this.hub)).tools;
      this.loadProblem = '';
      this.status = 'ready';
    } catch (reason) {
      this.loadProblem = messageOf(reason);
      this.status = 'failed';
    }
  }

  change(change: McpChange): Promise<void> {
    this.#queue = [...this.#queue, change];
    this.#tail = this.#tail.then(() => this.#save(change));
    return this.#tail;
  }

  dismissProblem(): void {
    this.problem = null;
  }

  async #save(change: McpChange): Promise<void> {
    try {
      const next = await this.shell.write(this.hub, change);
      this.#confirmed = next.tools;
      if (this.problem?.control === controlOf(change)) this.problem = null;
      this.saved = describeChange(change, next.tools);
    } catch (reason) {
      this.saved = '';
      this.problem = { control: controlOf(change), attempt: nameChange(change), reason: messageOf(reason) };
    } finally {
      this.#queue = this.#queue.filter((queued) => queued !== change);
    }
  }
}
