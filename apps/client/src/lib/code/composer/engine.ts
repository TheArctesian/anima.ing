import type { CallResult } from '$lib/agent-threads/live/live.svelte';
import type { RunRow } from '$lib/agent-threads/live/run';
import type { Attachment } from './attachments/attach';
import { CAN, cannot, type Can } from './draft';
import { builtInCommands } from './mentions/lists/commands';
import type { FoundPath, MentionSources } from './mentions/sources';
import type { Catalogue, Choice } from './picker/catalogue';
import type { HeldQueue, QueuePhase } from './queue/held.svelte';
import type { Queued } from './queue/queue';
import { catalogueOf, changeOf, type EffortOfRow, type ProgramModel, type SetCall } from './run/choice';
import { upload, type AttachCall, type Sendable, type Uploaded, type UploadWatch } from './run/upload';

/** How a call to the engine ended; the words of a failure go under the editor. */
export type Outcome = { readonly ok: true } | { readonly ok: false; readonly words: string };

/** What goes to the program: the prompt as Markdown and what is attached to it. */
export interface Message {
  readonly prompt: string;
  readonly attachments: readonly Attachment[];
}

/**
 * Everything the composer needs of the program that runs a chat. It is the one place where the page meets what the
 * runner can and cannot do: a part the runner lacks is a `Can` with the reason, and its control is drawn but cannot
 * be used. The composer reads no store and makes no call of its own.
 */
export interface ComposerEngine {
  /** `running`: a turn is on; `ended`: the program is gone and the chat takes no message. */
  readonly phase: 'idle' | 'running' | 'ended';
  /** Why the chat takes no message, when it does not. */
  readonly ended: string | null;
  /** Starts a turn, or goes into the running one. */
  readonly send: (message: Message) => Promise<Outcome>;
  /** Stops the running turn; the program stays. */
  readonly interrupt: () => Promise<Outcome>;
  readonly attach: Can;
  /** Gives a file to the program's side, part by part; left out by an engine that needs no file before the send. */
  readonly upload?: (file: Sendable, mime: string, watch: UploadWatch) => Promise<Uploaded>;
  /** Gives up a file that was given and will not go with a message. */
  readonly discard?: (remote: string) => void;
  readonly dictate: Can;
  /** A message into the running turn. */
  readonly steer: Can;
  /** A message that waits for the turn to end. */
  readonly queue: Can;
  readonly queued: readonly Queued[];
  /** Why the queue sends nothing on its own, when it does not: after a stop, or a message that did not go. */
  readonly queueWaits?: string | null;
  /** Tells which queued message is open in the composer (null: none), so that it is not sent while it is changed. */
  readonly opened?: (id: string | null) => void;
  /** The composer went away while a queued message was open: its change is lost, and the queue must not go on alone. */
  readonly abandoned?: () => void;
  readonly enqueue: (message: Message) => Promise<Outcome>;
  readonly requeue: (id: string, message: Message) => Promise<Outcome>;
  readonly unqueue: (id: string) => Promise<Outcome>;
  /** Sends a queued message now: into the running turn, or as the next one. */
  readonly sendQueued: (id: string) => Promise<Outcome>;
  /** The model, the effort and the permissions of the chat, and whether they can be changed now. */
  readonly pick: Can;
  readonly catalogue: Catalogue;
  readonly choice: Choice;
  /** Takes the new choice; the words of a failure when the program refuses it. */
  readonly choose: (next: Choice) => Promise<Outcome>;
  /** What `@`, `/` and `$` list. */
  readonly sources: MentionSources;
}

/** The three calls of the runner for a chat that it drives. */
export interface RunCalls {
  /** `files`: the ids of the runner for the files of this prompt (TH111); left out when it has none. */
  readonly onSend: (text: string, files?: readonly string[]) => Promise<CallResult>;
  readonly onInterrupt: () => Promise<CallResult>;
  /** The runner's `attach`, `attach_more` and `unattach` for this chat (TH111); without them nothing is attached. */
  readonly onAttach?: AttachCall | undefined;
  /** The calls of the runner's own queue (`live.call`), for the chat of this composer; without them the window holds the queue. */
  readonly onQueue?: QueueCall | undefined;
  /** The runner's `set` call for this chat (TH110); without it the model and the permissions cannot be changed. */
  readonly onSet?: SetCall | undefined;
  /** The models of the chat's program, as the runner's `models` call names them (TH104); without them the model stays. */
  readonly models?: readonly ProgramModel[] | undefined;
  /** The runner's list of the files of the chat's folder (`live.files`); left out by a page whose runner has none. */
  readonly onFiles?: FileSearch | undefined;
}

/**
 * The two fields of a row that the runner's queue adds (TH109), as `live/queue.ts` of that task reads them. They are
 * named here so that the composer does not wait for that task: a row without them has an empty queue.
 */
export interface QueueOfRow {
  readonly queued?: readonly { readonly id: string; readonly text: string }[];
  readonly lastTurn?: 'done' | 'failed' | 'interrupted' | null;
}

/** A call of the runner's queue, without the thread: the page adds it. */
export type QueueAct =
  | { readonly call: 'queue'; readonly text: string }
  | { readonly call: 'requeue'; readonly id: string; readonly text: string }
  | { readonly call: 'unqueue'; readonly id: string }
  | { readonly call: 'send_queued'; readonly id: string };
export type QueueCall = (act: QueueAct) => Promise<CallResult>;

/** The answer of the runner's file list, as `live.files` gives it. */
export type FileSearch = (query: string, most: number) => Promise<{ readonly ok: true; readonly paths: readonly FoundPath[]; readonly more: boolean } | { readonly ok: false; readonly words: string }>;

/** The most characters of a query that are sent: the runner refuses one over 256 bytes, and a character is at most four. */
const QUERY_MOST = 64;

/** The lists of a chat whose runner searches its folder; a search that is refused is one that failed. */
function searching(search: FileSearch, lists: MentionSources): MentionSources {
  return {
    ...lists,
    search: CAN,
    files: async (query, most) => {
      const found = await search(query.slice(0, QUERY_MOST), most);
      if (!found.ok) throw new Error(found.words);
      return found.paths;
    },
  };
}

const NO_ATTACH = 'The runner takes no attachments yet.';
const NO_FILES_QUEUED: Outcome = { ok: false, words: 'A message that waits in the queue takes no files yet. Remove the files, or send the message into the running turn.' };
const NO_DICTATE = 'Voice input is not built yet.';
const NO_QUEUE_WORDS = 'The runner keeps no queue yet: a message goes into the running turn.';
const STOPPED_TURN = 'The last turn was stopped, so the queue waits. Send a message, or press Send now.';
const FAILED_TURN = 'The last turn failed, so the queue waits. Send a message, or press Send now.';
const STOPPED = 'You stopped the turn, so the queue waits. Send a message, or press Send now.';
const NO_SEARCH = 'The runner cannot search the folder of a chat yet.';
const NO_PICK = 'The model and the permissions are set when a chat starts; the runner cannot change them after.';
const NO_COMMAND = 'The runner has no such command yet.';

function outcomeOf(result: CallResult): Outcome {
  return result.ok ? { ok: true } : { ok: false, words: result.words };
}

/** One list for every row, so that what reads it is not told of a change at each update of the chat. */
const SOURCES: MentionSources = {
  commands: builtInCommands({ model: cannot(NO_PICK), clear: cannot(NO_COMMAND) }),
  skills: [],
  files: () => Promise.resolve([]),
  search: cannot(NO_SEARCH),
};

/** The same lists for a chat whose runner takes a change of the mode and the effort: `/model` opens the picker. */
const SOURCES_SET: MentionSources = { ...SOURCES, commands: builtInCommands({ model: CAN, clear: cannot(NO_COMMAND) }) };

/** The phase of a chat by the state of its run: a program that waits for an answer is still in its turn. */
export function phaseOf(row: RunRow): QueuePhase {
  if (row.state === 'stopped') return 'ended';
  return row.state === 'working' || row.state === 'starting' || row.state === 'needs_you' ? 'running' : 'idle';
}

type QueueParts = Pick<ComposerEngine, 'queue' | 'queued' | 'queueWaits' | 'opened' | 'abandoned' | 'enqueue' | 'requeue' | 'unqueue' | 'sendQueued'>;

const refused = (): Promise<Outcome> => Promise.resolve({ ok: false, words: NO_QUEUE_WORDS });
const NO_QUEUE: QueueParts = { queue: cannot(NO_QUEUE_WORDS), queued: [], enqueue: refused, requeue: refused, unqueue: refused, sendQueued: refused };

/** Why the runner's queue sends nothing on its own, by how the last turn ended; null while it goes on. */
function runnerWaits(row: RunRow & QueueOfRow): string | null {
  if (row.queued === undefined || row.queued.length === 0 || phaseOf(row) !== 'idle') return null;
  if (row.lastTurn === 'interrupted') return STOPPED_TURN;
  return row.lastTurn === 'failed' ? FAILED_TURN : null;
}

/**
 * The queue calls of the engine over the runner's own queue (TH109): the runner holds the prompts, sends the first
 * when a turn ended well with nothing open, and stops after an interrupt or a failed turn until the person sends.
 */
function runnerQueue(row: RunRow & QueueOfRow, call: QueueCall, send: (prompt: string) => Promise<Outcome>): QueueParts {
  const act = async (action: QueueAct): Promise<Outcome> => outcomeOf(await call(action));
  return {
    queue: CAN,
    queued: (row.queued ?? []).map((prompt) => ({ id: prompt.id, prompt: prompt.text, saving: false })),
    queueWaits: runnerWaits(row),
    // The turn can end between the press and this call: what would wait for nothing goes now.
    enqueue: (message) => (phaseOf(row) === 'running' ? act({ call: 'queue', text: message.prompt }) : send(message.prompt)),
    requeue: (id, message) => act({ call: 'requeue', id, text: message.prompt }),
    unqueue: (id) => act({ call: 'unqueue', id }),
    sendQueued: (id) => act({ call: 'send_queued', id }),
  };
}

/** The queue calls of the engine, over the queue that this window holds for the chat. */
function queueOf(queue: HeldQueue, running: boolean, send: (prompt: string) => Promise<Outcome>): QueueParts {
  return {
    queue: CAN,
    queued: queue.messages,
    queueWaits: queue.waits,
    opened: (id) => {
      queue.opened(id);
    },
    abandoned: () => {
      queue.abandoned();
    },
    // The turn can end between the press and this call: what would wait for nothing goes now.
    enqueue: (message) => (running ? Promise.resolve(queue.add(message.prompt)) : send(message.prompt)),
    requeue: (id, message) => Promise.resolve(queue.change(id, message.prompt)),
    unqueue: (id) => Promise.resolve(queue.remove(id)),
    sendQueued: (id) => queue.sendNow(id, send),
  };
}

/**
 * The engine of a chat that the runner of this computer drives (TH14). It sends, steers and interrupts. It queues in
 * the runner when the page gives the runner's queue calls, else in this window. It attaches when the page gives the runner's file calls; it has no dictation; it changes the
 * mode and the effort when the page gives the runner's `set` call; it searches the chat's folder when the page gives it the runner's file list.
 *
 * The queue is given, not looked up here: state that is made inside the computation that reads it is not followed.
 */
export function runEngine(row: RunRow & QueueOfRow & EffortOfRow, calls: RunCalls, queue: HeldQueue | null): ComposerEngine {
  const phase = phaseOf(row);
  const send = async (prompt: string): Promise<Outcome> => outcomeOf(await calls.onSend(prompt));
  const sendAll = async (message: Message): Promise<Outcome> => {
    const files = message.attachments.flatMap((attachment) => (attachment.remote === undefined ? [] : [attachment.remote]));
    const outcome = files.length === 0 ? await send(message.prompt) : outcomeOf(await calls.onSend(message.prompt, files));
    // The person started a turn themself: the queue goes on after it.
    if (outcome.ok) queue?.release();
    return outcome;
  };
  const attach = calls.onAttach;
  const set = calls.onSet;
  const lists = set === undefined ? SOURCES : SOURCES_SET;
  // The runner's own queue when the page gives its calls (TH109); else the one this window holds; else none.
  const queued = calls.onQueue !== undefined ? runnerQueue(row, calls.onQueue, send) : queue !== null ? queueOf(queue, phase === 'running', send) : NO_QUEUE;
  return {
    phase,
    ended: phase === 'ended' ? (row.ended ?? 'The program of this chat ended.') : null,
    send: sendAll,
    interrupt: async () => {
      queue?.hold(STOPPED);
      const outcome = outcomeOf(await calls.onInterrupt());
      if (!outcome.ok && queue?.waits === STOPPED) queue.release();
      return outcome;
    },
    attach: attach === undefined ? cannot(NO_ATTACH) : CAN,
    ...(attach === undefined ? {} : { upload: (file, mime, watch) => upload(attach, file, mime, watch), discard: (remote) => void attach({ call: 'unattach', attachment: remote }).catch(() => undefined) }),
    dictate: cannot(NO_DICTATE),
    steer: CAN,
    ...queued,
    // The queue of the runner and the one of this window hold words only. With no turn on, a message that would wait goes now, with its files.
    enqueue: (message) => (message.attachments.length === 0 ? queued.enqueue(message) : phase === 'running' ? Promise.resolve(NO_FILES_QUEUED) : sendAll(message)),
    requeue: (id, message) => (message.attachments.length === 0 ? queued.requeue(id, message) : Promise.resolve(NO_FILES_QUEUED)),
    pick: set === undefined ? cannot(NO_PICK) : CAN,
    ...catalogueOf(row, set !== undefined, calls.models),
    choose: async (next) => {
      const change = changeOf(row, next);
      return set === undefined || change === null ? { ok: true } : outcomeOf(await set(change));
    },
    sources: calls.onFiles === undefined ? lists : searching(calls.onFiles, lists),
  };
}
