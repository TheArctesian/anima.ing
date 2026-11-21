import type { CallResult } from '$lib/agent-threads/live/live.svelte';
import type { Mode, RunRow } from '$lib/agent-threads/live/run';
import { programName } from '$lib/agent-threads/thread';
import type { Attachment } from './attachments/attach';
import { CAN, cannot, type Can } from './draft';
import { builtInCommands } from './mentions/lists/commands';
import type { MentionSources } from './mentions/sources';
import { PERMISSION_MODES, type Catalogue, type Choice } from './picker/catalogue';
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
const NO_QUEUE = 'The runner keeps no queue yet: a message goes into the running turn.';
const NO_PICK = 'The model and the permissions are set when a chat starts; the runner cannot change them after.';
const NO_COMMAND = 'The runner has no such command yet.';

const MODES: Readonly<Record<Mode, string>> = { ask: 'approval-required', accept_edits: 'auto-accept-edits', full_access: 'full-access' };

function outcomeOf(result: CallResult): Outcome {
  return result.ok ? { ok: true } : { ok: false, words: result.words };
}

const refused = (words: string) => (): Promise<Outcome> => Promise.resolve({ ok: false, words });

/** The one model a driven chat has, as a catalogue: what the picker's trigger shows. */
function catalogueOf(row: RunRow): { catalogue: Catalogue; choice: Choice } {
  const model = row.model ?? 'default';
  return {
    catalogue: {
      providers: [{ id: row.program, name: programName(row.program), icon: 'bot', can: CAN }],
      models: [{ id: model, provider: row.program, label: row.model ?? 'Default model', favourite: false, can: CAN, efforts: [], speeds: [], toggles: [] }],
      modes: PERMISSION_MODES,
    },
    choice: { provider: row.program, model, effort: null, speed: null, toggles: [], mode: MODES[row.mode] },
  };
}

/**
 * The engine of a chat that the runner of this computer drives (TH14). It sends, steers and interrupts; it has no
 * attachments, no queue, no change of the model after the start, no search of the workspace and no dictation.
 */
export function runEngine(row: RunRow, calls: RunCalls): ComposerEngine {
  const working = row.state === 'working' || row.state === 'starting' || row.state === 'needs_you';
  const gone = row.state === 'stopped';
  return {
    phase: gone ? 'ended' : working ? 'running' : 'idle',
    ended: gone ? (row.ended ?? 'The program of this chat ended.') : null,
    send: async (message) => outcomeOf(await calls.onSend(message.prompt)),
    interrupt: async () => outcomeOf(await calls.onInterrupt()),
    attach: cannot(NO_ATTACH),
    dictate: cannot(NO_DICTATE),
    steer: CAN,
    queue: cannot(NO_QUEUE),
    queued: [],
    enqueue: refused(NO_QUEUE),
    requeue: refused(NO_QUEUE),
    unqueue: refused(NO_QUEUE),
    sendQueued: refused(NO_QUEUE),
    pick: cannot(NO_PICK),
    ...catalogueOf(row),
    choose: () => undefined,
    sources: {
      commands: builtInCommands({ model: cannot(NO_PICK), clear: cannot(NO_COMMAND) }),
      skills: [],
      files: () => Promise.resolve([]),
    },
  };
}
