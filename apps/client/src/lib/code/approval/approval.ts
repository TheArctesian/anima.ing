import type { IconName } from '$lib/icons/icons';
import type { MenuEntry } from '../kit/menu';

/** What an agent asks to do (03 section A.10). */
export type ApprovalKind = 'command' | 'file-change' | 'file-read' | 'permission' | 'app-access';

export interface ApprovalLook {
  /** The row's accessible name. */
  readonly name: string;
  readonly icon: IconName;
}

export const APPROVAL_LOOKS: Readonly<Record<ApprovalKind, ApprovalLook>> = {
  command: { name: 'Command approval', icon: 'carbon:terminal' },
  'file-change': { name: 'File change approval', icon: 'carbon:edit' },
  'file-read': { name: 'File read approval', icon: 'carbon:document' },
  permission: { name: 'App permission approval', icon: 'carbon:tool-kit' },
  'app-access': { name: 'App access approval', icon: 'carbon:connect' },
};

/** The answers shown in the row itself; every other answer goes in the "More" menu. */
export type InlineDecision = 'decline' | 'accept';

/** The "More" menu when the agent sends no answers of its own. */
export const DEFAULT_MORE: readonly MenuEntry[] = [
  { kind: 'choice', id: 'cancel', label: 'Cancel' },
  { kind: 'choice', id: 'acceptForSession', label: 'Always allow this session' },
];

/** The words that take the place of the answers when the agent's process has stopped. */
export const PROCESS_GONE = 'Provider process is gone. Interrupt or restart the run to respond.';
