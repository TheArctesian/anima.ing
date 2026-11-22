import type { CodeIconName } from '../icons/icons';

/**
 * What the details card of a thread shows (07-details-card.md). Each part is absent when the engine does not give it;
 * the card then leaves the row out, as T3 Code does. No drawing here.
 */

/** A machine that a thread can run on. */
export interface Environment {
  readonly id: string;
  readonly label: string;
  readonly kind: 'server' | 'cloud' | 'linux' | 'desktop' | 'laptop';
}

export type WorkspaceMode = 'local' | 'worktree';

/** Where the thread runs: the machine and the folder (07 section 1.1). */
export interface RunContext {
  readonly environments: readonly Environment[];
  /** The machine of the thread; absent when none matched. */
  readonly environment?: string | undefined;
  readonly mode: WorkspaceMode;
  /** The folder of the worktree, when the thread has one. */
  readonly worktreePath?: string | undefined;
  /** The full path of the workspace, for the tip and for "Copy full path". */
  readonly path?: string | undefined;
  /** The run context cannot change now: the row is text, not a control. */
  readonly locked: boolean;
}

/** An editor on the machine of the person (07 section 1.2). */
export interface Editor {
  readonly id: string;
  readonly label: string;
}

export interface OpenIn {
  readonly editors: readonly Editor[];
  /** The editor that the main half opens; absent when none is chosen. */
  readonly preferred?: string | undefined;
  /** Why no editor can open the folder; the rows of the menu are then off and this line says why. */
  readonly blocked?: string | undefined;
}

export type ScriptIcon = 'play' | 'test' | 'lint' | 'configure' | 'build' | 'debug';

/** A command of the project (07 section 1.3). */
export interface Script {
  readonly id: string;
  readonly name: string;
  readonly icon: ScriptIcon;
  readonly shortcut?: string | undefined;
  readonly onSetup?: boolean | undefined;
  readonly onSettle?: boolean | undefined;
}

export interface Scripts {
  readonly scripts: readonly Script[];
  /** The script of the main half: the last one that the person ran. */
  readonly primary?: string | undefined;
  /** Why this connection cannot run a script. */
  readonly blocked?: string | undefined;
}

/** A branch, a tag or a remote branch in the picker (07 section 2.2). */
export interface Ref {
  readonly name: string;
  readonly current?: boolean | undefined;
  readonly worktree?: boolean | undefined;
  readonly remote?: boolean | undefined;
  readonly isDefault?: boolean | undefined;
  readonly disabled?: boolean | undefined;
}

export interface Branch {
  /** The branch of the thread; absent when none is known. */
  readonly name?: string | undefined;
  /** The thread starts a new worktree from this branch; the label is then "From {branch}". */
  readonly base?: boolean | undefined;
  readonly fromRemote?: boolean | undefined;
  /** The first load of the refs, or a branch action, runs. */
  readonly busy?: boolean | undefined;
}

export type PullRequestState = 'open' | 'draft' | 'closed' | 'merged';

export type PullRequestAction = 'resolve' | 'ready' | 'fix' | 'merge';

/** A pull request of the branch or one that the thread links (07 section 2.3). */
export interface PullRequest {
  readonly number: number;
  readonly title?: string | undefined;
  readonly state?: PullRequestState | undefined;
  readonly checks?: { readonly done: number; readonly total: number; readonly failed: number } | undefined;
  readonly conflicting?: boolean | undefined;
  /** The thread wakes when the checks end, someone comments or the branch conflicts. */
  readonly watching?: boolean | undefined;
  readonly canMarkReady?: boolean | undefined;
  readonly canMerge?: boolean | undefined;
  /** The action that runs now. */
  readonly pending?: PullRequestAction | undefined;
}

export interface Issue {
  readonly key: string;
  readonly title: string;
  readonly href: string;
  readonly comments?: number | undefined;
}

/** A local server that a terminal or the agent of the thread started (07 section 3). */
export interface Server {
  readonly id: string;
  readonly port: number;
  readonly startedBy: 'agent' | 'terminal';
  readonly process?: string | undefined;
  readonly forwardedTo?: number | undefined;
  readonly canForward?: boolean | undefined;
  readonly canStop?: boolean | undefined;
}

/** A scheduled task of the thread (07 section 4). */
export interface Automation {
  readonly id: string;
  readonly title: string;
  readonly schedule: string;
  readonly enabled: boolean;
  readonly lastRun: 'never' | 'running' | 'succeeded' | 'failed';
  /** "in 2 hours": when it runs next. */
  readonly next?: string | undefined;
  readonly webhook?: boolean | undefined;
}

export type RelatedKind = 'parent' | 'fork' | 'transfer' | 'subagent' | 'side_chat';

export type RelatedStatus =
  | 'starting'
  | 'running'
  | 'queued'
  | 'waiting'
  | 'done'
  | 'failed'
  | 'stopped'
  | 'reverted'
  | 'consumed'
  | 'superseded'
  | 'idle'
  | 'unknown';

/** A thread that this thread comes from or started (07 section 5). */
export interface Related {
  readonly id: string;
  readonly kind: RelatedKind;
  readonly title: string;
  readonly status: RelatedStatus;
  /** How long a subagent has run, in seconds. */
  readonly seconds?: number | undefined;
  /** The machine of a subagent that runs elsewhere. */
  readonly machine?: string | undefined;
  readonly canStop?: boolean | undefined;
}

/** One window of a limit of the account of the thread (07 section 6). */
export interface Limit {
  readonly label: string;
  /** How much is left, 0 to 100. */
  readonly left: number;
  /** How much of the time of the window is left, 0 to 100; the pace mark. */
  readonly timeLeft?: number | undefined;
  readonly pace?: 'ahead' | 'on' | 'under' | undefined;
  /** Seconds to the reset, and the time of the reset in words. */
  readonly resetsIn?: number | undefined;
  readonly resetsAt?: string | undefined;
}

export type Usage = { readonly limits: readonly Limit[] } | { readonly unavailable: string };

/** The context window of the thread (07 section 7). */
export interface ContextWindow {
  readonly used: number;
  readonly max?: number | undefined;
  /** The count of tokens at which the program compacts by itself. */
  readonly compactsAt?: number | undefined;
}

/** All that the card can show. */
export interface Details {
  /** A thread that has not started: no servers, automations or related threads. */
  readonly draft?: boolean | undefined;
  readonly runContext?: RunContext | undefined;
  readonly openIn?: OpenIn | undefined;
  readonly scripts?: Scripts | undefined;
  readonly branch?: Branch | undefined;
  readonly pullRequests?: readonly PullRequest[] | undefined;
  readonly issues?: readonly Issue[] | undefined;
  readonly servers?: readonly Server[] | undefined;
  readonly automations?: readonly Automation[] | undefined;
  readonly automationsError?: string | undefined;
  readonly related?: readonly Related[] | undefined;
  readonly usage?: Usage | undefined;
  readonly context?: ContextWindow | undefined;
}

export const SCRIPT_ICONS: Readonly<Record<ScriptIcon, CodeIconName>> = {
  play: 'play',
  test: 'flask-conical',
  lint: 'list-checks',
  configure: 'wrench',
  build: 'hammer',
  debug: 'bug',
};

export const MACHINE_ICONS: Readonly<Record<Environment['kind'], CodeIconName>> = {
  server: 'server',
  cloud: 'cloud',
  linux: 'terminal',
  desktop: 'monitor',
  laptop: 'laptop',
};
