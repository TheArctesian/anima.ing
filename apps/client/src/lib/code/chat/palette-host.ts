import { goto } from '$app/navigation';
import { resolve } from '$app/paths';
import type { ListedRow } from '$lib/agent-threads/listing/listing';
import { folderName } from '$lib/agent-threads/thread';
import type { PaletteChat, PaletteHost } from '../palette/adapter';

/** The threads of the inbox as the command palette lists them. */
export function paletteChats(rows: readonly ListedRow[]): PaletteChat[] {
  return rows.map((row) => ({
    id: row.id,
    title: row.title ?? 'Untitled thread',
    ...(row.folder === null ? {} : { project: folderName(row.folder) }),
    ...(row.branch === null ? {} : { branch: row.branch }),
    at: (row.last ?? row.started)?.getTime() ?? 0,
  }));
}

/**
 * The chats used last, newest first (T3 Code's "Recent Threads"). The page keeps no time of opening, so a chat's last
 * activity stands for its last use.
 */
export function recentChats(chats: readonly PaletteChat[]): string[] {
  return chats.toSorted((left, right) => right.at - left.at).map((chat) => chat.id);
}

/**
 * Puts the caret at the end of the composer (T3 Code's `composer.focusAtEnd`); false when there is no composer that can
 * take the focus, as behind a dialog.
 */
export function focusComposerAtEnd(): boolean {
  const editor = document.querySelector<HTMLElement>('.code-editor-page[contenteditable="true"]');
  if (editor === null || editor.closest('[inert], [aria-modal="true"]') !== null) return false;
  editor.focus();
  if (document.activeElement !== editor) return false;
  const range = document.createRange();
  range.selectNodeContents(editor);
  range.collapse(false);
  const selection = window.getSelection();
  selection?.removeAllRanges();
  selection?.addRange(range);
  return true;
}

/** What the Code page gives the palette: its threads, the open one, and the actions it runs. */
export interface PageForPalette {
  readonly rows: readonly ListedRow[] | null;
  readonly failed: boolean;
  readonly chosen: string | null;
  readonly newThread: (() => void) | undefined;
  /** Opens a draft with the "Add project" palette. */
  readonly addProject: (() => void) | undefined;
  readonly open: (id: string) => void;
  /** Goes to the Settings page, where Code has its appearance group. */
  readonly settings: () => void;
}

export function paletteHost(page: PageForPalette): PaletteHost {
  const chats = paletteChats(page.rows ?? []);
  return {
    chats,
    chatsState: page.failed ? 'failed' : page.rows === null ? 'loading' : 'ready',
    recent: recentChats(chats),
    ...(page.chosen === null ? {} : { openChat: page.chosen }),
    runs: {
      'settings.open': page.settings,
      'usage.open': () => void goto(resolve('/agents/usage')),
      ...(page.newThread === undefined ? {} : { 'chat.new': page.newThread }),
      ...(page.addProject === undefined ? {} : { 'project.add': page.addProject }),
    },
    openChatById: page.open,
    returnFocus: focusComposerAtEnd,
  };
}
