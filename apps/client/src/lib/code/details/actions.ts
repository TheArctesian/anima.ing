import type { Issue, PullRequestAction, Related, Server, WorkspaceMode } from './model';

/**
 * What the engine can do for the details card. Each action is absent until the engine has it: the card then shows the
 * control switched off. The owner shows a failure as a toast; the card keeps no error state of its own.
 */
export interface Actions {
  /** The modifier key of this system in words, for a tip: "⌘" or "Ctrl". */
  readonly modifier?: string;
  readonly copy?: (text: string) => void;
  readonly environment?: (id: string) => void;
  readonly mode?: (mode: WorkspaceMode) => void;
  readonly openShortcut?: string;
  readonly openEditor?: () => void;
  readonly preferEditor?: (id: string) => void;
  readonly runScript?: (id: string) => void;
  readonly addScript?: () => void;
  readonly editScript?: (id: string) => void;
  readonly loadRefs?: () => void;
  readonly pickRef?: (name: string) => void;
  readonly createRef?: (name: string) => void;
  readonly fromRemote?: (on: boolean) => void;
  readonly openPullRequest?: (number: number) => void;
  readonly unwatch?: (number: number) => void;
  readonly openChecks?: (number: number) => void;
  readonly pullRequestAction?: (number: number, action: PullRequestAction) => void;
  readonly openIssue?: (issue: Issue) => void;
  readonly openServer?: (server: Server, outside: boolean) => void;
  readonly forward?: (server: Server) => void;
  readonly stopServer?: (server: Server) => void;
  readonly busyServer?: string;
  readonly manageAutomations?: () => void;
  readonly editAutomation?: (id: string) => void;
  readonly runAutomation?: (id: string) => void;
  readonly enableAutomation?: (id: string, on: boolean) => void;
  readonly busyAutomation?: string;
  readonly openThread?: (row: Related, other: boolean) => void;
  readonly stopAgent?: (row: Related) => void;
  readonly stoppingAgent?: string;
  readonly disconnect?: () => void;
  readonly openUsage?: () => void;
}
