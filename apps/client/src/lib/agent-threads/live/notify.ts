/** One notification for each passage to a state that needs the person, none while the thread is open in front (THR-059). */

import { folderName } from '../thread';
import type { RequestKind, Update } from './run';
import type { LiveThread } from './timeline';

export const ASK_WORDS: Readonly<Record<RequestKind, string>> = {
  command: 'Asks to run a command',
  file_change: 'Asks to change a file',
  tool: 'Asks to use a tool',
  question: 'Asks a question',
};

export interface Notice {
  readonly thread: string;
  readonly title: string;
  readonly words: string;
}

export interface Looking {
  readonly front: boolean;
  readonly chosen: string | null;
}

export function noticeFor(before: LiveThread | undefined, after: LiveThread, update: Update, looking: Looking): Notice | null {
  if (looking.front && looking.chosen === after.row.id) return null;
  const title = after.row.title === '' ? folderName(after.row.folder) : after.row.title;
  const change = update.change;
  if (change.change === 'asked' && (before?.asks.length ?? 0) === 0) {
    return { thread: after.row.id, title, words: ASK_WORDS[change.ask.kind] };
  }
  if (change.change === 'state' && change.state === 'idle' && before?.row.state === 'working') {
    return { thread: after.row.id, title, words: 'Your turn' };
  }
  return null;
}
