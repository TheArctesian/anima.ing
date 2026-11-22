import type { OpenAsk } from '$lib/agent-threads/live/timeline';
import { folded, type Turn } from './turns';

/**
 * What the live regions of the conversation say (finding 3 and 4 of the accessibility review): one line when a turn
 * starts, one when it ends, and one for each request of the agent. Never the reply as it streams, so a screen reader is
 * not flooded.
 */

/** A line for the polite region; a new `key` is a new thing to say. */
export interface Spoken {
  readonly key: string;
  readonly words: string;
}

/** The most characters of a line: the start of the reply or of the request, not all of it. */
const MOST = 160;

function cut(text: string): string {
  const line = text.replace(/\s+/g, ' ').trim();
  return line.length <= MOST ? line : `${line.slice(0, MOST - 1)}…`;
}

/** The first sentence of a reply, at most {@link MOST} characters. */
function firstSentence(text: string): string {
  const flat = text.replace(/\s+/g, ' ').trim();
  const end = flat.search(/[.!?](\s|$)/);
  return cut(end < 0 ? flat : flat.slice(0, end + 1));
}

/** What the polite region says of the last turn: that the agent works, that it replied, or that the turn stopped. */
export function spokenOf(turns: readonly Turn[]): Spoken | null {
  const turn = turns.at(-1);
  if (turn === undefined) return null;
  if (turn.running) return { key: `${turn.key}:working`, words: 'Agent is working.' };
  if (turn.stopped) return { key: `${turn.key}:stopped`, words: 'Stopped.' };
  const answer = folded(turn).answer;
  if (answer?.kind !== 'reply' || answer.text.trim() === '') return { key: `${turn.key}:ended`, words: 'Agent finished.' };
  return { key: `${turn.key}:ended`, words: `Agent replied: ${firstSentence(answer.text)}` };
}

const ASKED: Readonly<Record<OpenAsk['ask']['kind'], string>> = {
  command: 'Approval needed to run a command',
  file_change: 'Approval needed to change a file',
  tool: 'Approval needed to use a tool',
  question: 'The agent asks',
};

/** The line for the assertive region when the agent asks the person something. */
export function askedWords(open: OpenAsk): string {
  const what = open.ask.kind === 'question' ? (open.ask.questions[0]?.text ?? open.ask.subject) : open.ask.subject;
  return `${ASKED[open.ask.kind]}: ${cut(what)}`;
}
