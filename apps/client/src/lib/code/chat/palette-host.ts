import type { ListedRow } from '$lib/agent-threads/listing/listing';
import { folderName } from '$lib/agent-threads/thread';
import type { PaletteChat, PaletteHost } from '../palette/adapter';

/** The threads of the inbox as the command palette lists them. */
export function paletteChats(rows: readonly ListedRow[]): PaletteChat[] {
  return rows.map((row) => ({
    id: row.id,
    title: row.title ?? 'Untitled thread',
    ...(row.folder === null ? {} : { project: folderName(row.folder) }),
    at: (row.last ?? row.started)?.getTime() ?? 0,
  }));
}

/** What the Code page gives the palette: its threads, the open one, and the actions it runs. */
export interface PageForPalette {
  readonly rows: readonly ListedRow[] | null;
  readonly failed: boolean;
  readonly chosen: string | null;
  readonly newThread: (() => void) | undefined;
  readonly open: (id: string) => void;
}

export function paletteHost(page: PageForPalette): PaletteHost {
  return {
    chats: paletteChats(page.rows ?? []),
    chatsState: page.failed ? 'failed' : page.rows === null ? 'loading' : 'ready',
    // The palette's "Recent chats" means opened last; the page keeps no such list yet.
    recent: [],
    ...(page.chosen === null ? {} : { openChat: page.chosen }),
    runs: page.newThread === undefined ? {} : { 'chat.new': page.newThread },
    openChatById: page.open,
  };
}
