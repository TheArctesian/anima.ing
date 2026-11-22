import type { CodeIconName } from '../icons/icons';

/**
 * The one seam between the command palette and the rest of Anima. The palette asks this file which actions it can run
 * and gives it the action to run; it never reaches the engine, the router or a store itself. An action whose page or
 * engine part is not built yet is shown disabled with the reason written here (TC6; the rows of Section 6.0 of
 * `docs/proposals/agent-threads.md`).
 */

/** The actions of T3 Code's root palette that Anima keeps (02 section 10.2), by the command id of the key table. */
export type PaletteAction =
  | 'chat.new'
  | 'chat.newWithoutProject'
  | 'project.add'
  | 'filePicker.toggle'
  | 'projectSearch.toggle'
  | 'chat.find'
  | 'thread.copyReference'
  | 'pullRequest.link'
  | 'session.restart'
  | 'pullRequests.open'
  | 'issues.open'
  | 'usage.open'
  | 'settings.open'
  /** Kept for hosts that still give it; the palette shows no row for it (T3 Code keeps the keys under Settings). */
  | 'keys.open'
  | 'workspace.focusNext'
  | 'workspace.focusPrevious';

export interface ActionLook {
  readonly id: PaletteAction;
  readonly label: string;
  readonly icon: CodeIconName;
  /** More words the search finds the action by. */
  readonly terms: readonly string[];
  /** Needs an open chat: hidden on the palette of the list alone. */
  readonly inChat?: boolean;
}

/**
 * T3 Code's root actions in its order (`CommandPalette.tsx`), with its Lucide icons and search terms; the ones that name
 * its product, its themes and its sub-views are left out. T3 Code has no row for the keys: they live under Settings.
 */
export const ACTIONS: readonly ActionLook[] = [
  { id: 'chat.new', label: 'New chat', icon: 'square-pen', terms: ['new thread', 'chat', 'create', 'draft'] },
  { id: 'chat.newWithoutProject', label: 'New chat without a project', icon: 'message-square-dashed', terms: ['new thread', 'no project', 'without project', 'none', 'chat'] },
  { id: 'chat.find', label: 'Find in current chat', icon: 'text-search', terms: ['find', 'search', 'messages', 'plans'], inChat: true },
  { id: 'thread.copyReference', label: 'Copy chat', icon: 'link', terms: ['copy', 'thread', 'thread id', 'reference'], inChat: true },
  { id: 'pullRequest.link', label: 'Link pull request to chat', icon: 'link-2', terms: ['link', 'pull request', 'pr', 'attach', 'stack'], inChat: true },
  { id: 'session.restart', label: 'Restart agent session', icon: 'rotate-ccw', terms: ['reload', 'agent'], inChat: true },
  { id: 'filePicker.toggle', label: 'Go to file', icon: 'file-search', terms: ['open', 'file'] },
  { id: 'projectSearch.toggle', label: 'Search project contents', icon: 'text-search', terms: ['grep', 'find'] },
  { id: 'project.add', label: 'Add project', icon: 'folder-plus', terms: ['folder', 'repository'] },
  { id: 'pullRequests.open', label: 'Open pull requests', icon: 'git-pull-request-arrow', terms: ['pr', 'github'] },
  { id: 'issues.open', label: 'Open issues', icon: 'circle-dot', terms: ['issues', 'issue', 'bugs', 'tracker', 'github', 'triage'] },
  { id: 'usage.open', label: 'Open usage', icon: 'chart-no-axes-column', terms: ['usage', 'use', 'tokens', 'cost', 'spend', 'limits', 'stats', 'analytics'] },
  { id: 'workspace.focusNext', label: 'Focus Next Workspace Area', icon: 'monitor', terms: ['accessibility', 'navigation', 'focus', 'F6'] },
  { id: 'workspace.focusPrevious', label: 'Focus Previous Workspace Area', icon: 'monitor', terms: ['accessibility', 'navigation', 'focus', 'F6'] },
  { id: 'settings.open', label: 'Open settings', icon: 'settings', terms: ['settings', 'preferences', 'configuration', 'keybindings'] },
];

/** A chat as the palette lists it. */
export interface PaletteChat {
  readonly id: string;
  readonly title: string;
  readonly project?: string;
  /** The Git branch it works on. */
  readonly branch?: string;
  /** The time of its last activity, in ms since the epoch. */
  readonly at: number;
}

/** What the palette reads and calls. The Code page gives one; the kit page gives a fixed one. */
export interface PaletteHost {
  /** Every chat, in any order. */
  readonly chats: readonly PaletteChat[];
  /** `ready`, or `loading` and `failed` while the list of chats has not come. */
  readonly chatsState: 'ready' | 'loading' | 'failed';
  /** The ids of the chats used last (written in or opened), newest first. */
  readonly recent: readonly string[];
  /** The chat that is open, if any: its actions show, it is left out of the recent chats, and a search lists it last. */
  readonly openChat?: string;
  /** The actions this host runs; the rest are disabled with their reason. */
  readonly runs: Partial<Record<PaletteAction, () => void>>;
  openChatById(id: string): void;
  /**
   * Where the focus goes when the palette closes and no action moved it (T3 Code's `finalFocus`: the end of the
   * composer); true when it went there. Without it, or when it gives false, the focus goes back to where it was.
   */
  readonly returnFocus?: () => boolean;
}

/** Why an action is not there yet: the part of the copy that builds it. */
const NOT_BUILT: Readonly<Record<PaletteAction, string>> = {
  'chat.new': 'Comes with the composer (TC4)',
  'chat.newWithoutProject': 'Comes with the composer (TC4)',
  'project.add': 'Projects are not in Code yet',
  'filePicker.toggle': 'Comes with the files panel (TC5)',
  'projectSearch.toggle': 'Comes with the files panel (TC5)',
  'chat.find': 'Comes with the timeline (TC3)',
  'thread.copyReference': 'Chat links are not built yet',
  'pullRequest.link': 'Comes with Git (TC7)',
  'session.restart': 'The engine cannot restart a session yet',
  'pullRequests.open': 'Comes with Git (TC7)',
  'issues.open': 'Comes with Git (TC7)',
  'usage.open': 'This page does not open agent usage',
  'settings.open': 'This page does not open settings',
  'keys.open': 'The keys are listed in Settings',
  'workspace.focusNext': 'Comes with the shell (TC2)',
  'workspace.focusPrevious': 'Comes with the shell (TC2)',
};

/** An action as the palette shows it: runnable, or disabled with the reason. */
export interface ShownAction extends ActionLook {
  readonly run?: () => void;
  readonly reason?: string;
}

/** The actions to show for `host`: those of a chat only while one is open. */
export function shownActions(host: PaletteHost): ShownAction[] {
  return ACTIONS.filter((action) => action.inChat !== true || host.openChat !== undefined).map((action) => {
    const run = host.runs[action.id];
    return run === undefined ? { ...action, reason: NOT_BUILT[action.id] } : { ...action, run };
  });
}
