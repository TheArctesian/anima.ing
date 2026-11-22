import type { CallResult } from '$lib/agent-threads/live/live.svelte';
import type { Mode, RunRow } from '$lib/agent-threads/live/run';
import { programName } from '$lib/agent-threads/thread';
import type { Attachment } from './attachments/attach';
import { CAN, cannot, type Can } from './draft';
import { builtInCommands } from './mentions/lists/commands';
import type { MentionSources } from './mentions/sources';
import { PERMISSION_MODES, type Catalogue, type Choice, type PermissionMode } from './picker/catalogue';
import type { HeldQueue, QueuePhase } from './queue/held.svelte';
import type { Queued } from './queue/queue';

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
  readonly choose: (next: Choice) => void;
  /** What `@`, `/` and `$` list. */
  readonly sources: MentionSources;
}

/** The three calls of the runner for a chat that it drives. */
export interface RunCalls {
  readonly onSend: (text: string) => Promise<CallResult>;
  readonly onInterrupt: () => Promise<CallResult>;
}

const NO_ATTACH = 'The runner takes no attachments yet.';
const NO_DICTATE = 'Voice input is not built yet.';
const STOPPED = 'You stopped the turn, so the queue waits. Send a message, or press Send now.';
const NO_SEARCH = 'The runner cannot search the folder of a chat yet.';
const NO_PICK = 'The model and the permissions are set when a chat starts; the runner cannot change them after.';
const NO_COMMAND = 'The runner has no such command yet.';

const MODES: Readonly<Record<Mode, string>> = { ask: 'approval-required', accept_edits: 'auto-accept-edits', full_access: 'full-access', plan: 'plan' };

/** The runner's own fourth mode, which T3 Code's four do not have (03 A.4.1): the program plans and changes nothing. */
const PLAN_MODE: PermissionMode = { id: 'plan', label: 'Plan', description: 'Plan the work; change no file and run no command.', icon: 'eye', can: CAN };

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

/** The one model a driven chat has, as a catalogue: what the picker's trigger shows. */
function catalogueOf(row: RunRow): { catalogue: Catalogue; choice: Choice } {
  const model = row.model ?? 'default';
  return {
    catalogue: {
      providers: [{ id: row.program, name: programName(row.program), icon: 'bot', can: CAN }],
      models: [{ id: model, provider: row.program, label: row.model ?? 'Default model', favourite: false, can: CAN, efforts: [], speeds: [], toggles: [] }],
      modes: [...PERMISSION_MODES, PLAN_MODE],
    },
    choice: { provider: row.program, model, effort: null, speed: null, toggles: [], mode: MODES[row.mode] },
  };
}

/** The phase of a chat by the state of its run: a program that waits for an answer is still in its turn. */
export function phaseOf(row: RunRow): QueuePhase {
  if (row.state === 'stopped') return 'ended';
  return row.state === 'working' || row.state === 'starting' || row.state === 'needs_you' ? 'running' : 'idle';
}

/** The queue calls of the engine, over the queue that this window holds for the chat. */
function queueOf(queue: HeldQueue, running: boolean, send: (prompt: string) => Promise<Outcome>): Pick<ComposerEngine, 'queue' | 'queued' | 'queueWaits' | 'opened' | 'abandoned' | 'enqueue' | 'requeue' | 'unqueue' | 'sendQueued'> {
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
 * The engine of a chat that the runner of this computer drives (TH14). It sends, steers and interrupts, and it
 * queues in this window (the runner keeps no queue). It has no attachments, no change of the model after the
 * start, no search of the workspace and no dictation.
 *
 * The queue is given, not looked up here: state that is made inside the computation that reads it is not followed.
 */
export function runEngine(row: RunRow, calls: RunCalls, queue: HeldQueue): ComposerEngine {
  const phase = phaseOf(row);
  const send = async (prompt: string): Promise<Outcome> => outcomeOf(await calls.onSend(prompt));
  return {
    phase,
    ended: phase === 'ended' ? (row.ended ?? 'The program of this chat ended.') : null,
    send: async (message) => {
      const outcome = await send(message.prompt);
      // The person started a turn themself: the queue goes on after it.
      if (outcome.ok) queue.release();
      return outcome;
    },
    interrupt: async () => {
      queue.hold(STOPPED);
      const outcome = outcomeOf(await calls.onInterrupt());
      if (!outcome.ok && queue.waits === STOPPED) queue.release();
      return outcome;
    },
    attach: cannot(NO_ATTACH),
    dictate: cannot(NO_DICTATE),
    steer: CAN,
    ...queueOf(queue, phase === 'running', send),
    pick: cannot(NO_PICK),
    ...catalogueOf(row),
    choose: () => undefined,
    sources: SOURCES,
  };
}
