import type { MenuEntry } from '../../kit/menu';

/**
 * The right-click menu of a row of the file tree (t3-ui-copy-2026-11-21/05, Section 2.6): its rows, and the words of
 * its failures. The tree draws it with the kit's MenuPanel at the pointer.
 */
export interface RowMenuAbilities {
  /** The server has a file manager, so the file can open with the system default. */
  readonly canOpen?: boolean;
  /** The words of "reveal in the file manager" for the system of the server; with none, the row is left out. */
  readonly revealLabel?: string | undefined;
  /** The editors that are installed. */
  readonly editors?: readonly { readonly id: string; readonly label: string }[];
}

export const ROW_OPEN = 'open';
export const ROW_REVEAL = 'reveal';
export const ROW_COPY_MENTION = 'copy-mention';
export const ROW_ADD_TO_CHAT = 'add-to-chat';
const EDITOR = 'editor:';

/** The rows in the order of T3 Code. "Open with" is a group of rows, since the kit's menu has no sub-menu. */
export function fileRowMenu(can: RowMenuAbilities = {}): readonly MenuEntry[] {
  const rows: MenuEntry[] = [];
  if (can.canOpen === true) rows.push({ kind: 'choice', id: ROW_OPEN, label: 'Open' });
  if (can.revealLabel !== undefined) rows.push({ kind: 'choice', id: ROW_REVEAL, label: can.revealLabel });
  const editors = can.editors ?? [];
  if (editors.length > 0) {
    if (rows.length > 0) rows.push({ kind: 'line' });
    rows.push({ kind: 'group', label: 'Open with' });
    rows.push(...editors.map((editor): MenuEntry => ({ kind: 'choice', id: `${EDITOR}${editor.id}`, label: editor.label })));
  }
  if (rows.length > 0) rows.push({ kind: 'line' });
  rows.push({ kind: 'choice', id: ROW_COPY_MENTION, label: 'Copy mention' }, { kind: 'choice', id: ROW_ADD_TO_CHAT, label: 'Add to chat' });
  return rows;
}

/** The editor that a chosen row names, or `null` for another row. */
export function rowMenuEditor(id: string): string | null {
  return id.startsWith(EDITOR) ? id.slice(EDITOR.length) : null;
}

/** A failure of a row of the menu, as the toast says it. */
export interface RowMenuProblem {
  readonly title: string;
  readonly description: string;
}

/** The words of each failure of the menu (2.6). `detail` is the error, the absolute path or the editor's name. */
export const ROW_MENU_PROBLEMS = {
  copy: (error?: string): RowMenuProblem => ({ title: 'Failed to copy mention', description: error ?? 'An error occurred.' }),
  noComposer: (): RowMenuProblem => ({ title: 'Unable to add to chat', description: 'Open a chat for this project and try again.' }),
  composerNotReady: (): RowMenuProblem => ({ title: 'Unable to add to chat', description: "The chat isn't ready to accept input right now." }),
  open: (path: string): RowMenuProblem => ({ title: 'Could not open file', description: path }),
  reveal: (path: string): RowMenuProblem => ({ title: 'Unable to reveal file', description: path }),
  editor: (path: string, editor: string): RowMenuProblem => ({ title: `Could not open in ${editor}`, description: path }),
} as const;
