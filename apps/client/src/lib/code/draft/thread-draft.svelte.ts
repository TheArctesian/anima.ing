import type { CallResult } from '$lib/agent-threads/live/live.svelte';
import type { Action } from '$lib/agent-threads/live/run';
import { CAN, cannot } from '../composer/draft';
import type { ComposerEngine, Outcome } from '../composer/engine';
import { builtInCommands } from '../composer/mentions/lists/commands';
import type { MentionSources } from '../composer/mentions/sources';
import { chosen, draftCatalogue, FIRST_CHOICE, type DraftChoice, type DraftModels } from './choice';

/** What a folder's own project files make the program do with no question (THR-046), as read for that folder. */
export interface FolderNotice {
  readonly folder: string;
  readonly lines: readonly string[];
}

const NO_FOLDER = 'Choose a project first: a thread runs in a folder of this computer.';
const SEND_AGAIN = 'This folder runs things by itself, as the notice above says. Send again to start the thread with them.';
const NOT_CHECKED = 'The project files of this folder could not be checked: they can make the program run commands with no question.';
const NOT_STARTED = 'The thread did not start.';
const NO_FILES = 'Files can be attached after the thread has started.';
const NO_QUEUE = 'The thread has not started: nothing is running for a message to wait for.';
const NO_SEARCH = 'The folder is searched after the thread has started.';
const NO_DICTATE = 'Voice input is not built yet.';
const NO_COMMAND = 'The runner has no such command yet.';

const SOURCES: MentionSources = {
  commands: builtInCommands({ model: CAN, clear: cannot(NO_COMMAND) }),
  skills: [],
  files: () => Promise.resolve([]),
  search: cannot(NO_SEARCH),
};

const refused = (words: string) => (): Promise<Outcome> => Promise.resolve({ ok: false, words });

/**
 * A thread that is not started yet (T3 Code's draft thread): the project it will run in and the program, model, effort
 * and access that the composer's pickers set. The first send starts it with the prompt; nothing else is asked.
 */
export class ThreadDraft {
  folder = $state<string | null>(null);
  /**
   * No project (T3 Code's scratch draft): the runner starts the thread in a folder of its own under `~/Anima Scratch`
   * (`start` with an empty folder, THR-041). A draft with no folder starts so; choosing a project ends it.
   */
  scratch = $state(false);
  choice = $state<DraftChoice>(FIRST_CHOICE);
  /** The notice of the last folder that was read, shown over the composer until the start. */
  notice = $state<FolderNotice | null>(null);

  constructor(folder: string | null) {
    this.folder = folder;
    this.scratch = folder === null;
  }

  /** The draft starts in `folder`, a project of this computer. */
  inFolder(folder: string): void {
    this.folder = folder;
    this.scratch = false;
  }

  /** The draft starts with no project, in a scratch folder the runner makes. */
  /** The "Add project" palette is open over the draft. */
  adding = $state(false);

  /** Opens the "Add project" palette (T3's Sources); the draft is kept. */
  addProject(): this {
    this.adding = true;
    return this;
  }

  withoutProject(): void {
    this.folder = null;
    this.scratch = true;
  }

  /** The composer's engine for this draft: `call` is the runner, `started` takes the new thread, `retry` asks for the models again. */
  engine(call: (action: Action) => Promise<CallResult>, started: (thread: string) => void, models: DraftModels, retry?: () => void): ComposerEngine {
    return {
      phase: 'idle',
      ended: null,
      send: async (message) => (message.attachments.length > 0 ? { ok: false, words: NO_FILES } : this.#start(message.prompt, call, started)),
      interrupt: () => Promise.resolve({ ok: true }),
      attach: cannot(NO_FILES),
      dictate: cannot(NO_DICTATE),
      steer: CAN,
      queue: cannot(NO_QUEUE),
      queued: [],
      enqueue: refused(NO_QUEUE),
      requeue: refused(NO_QUEUE),
      unqueue: refused(NO_QUEUE),
      sendQueued: refused(NO_QUEUE),
      pick: CAN,
      ...draftCatalogue(this.choice, models),
      ...(retry === undefined ? {} : { askModels: retry }),
      choose: (next) => {
        this.choice = chosen(this.choice, next);
        return Promise.resolve({ ok: true });
      },
      sources: SOURCES,
    };
  }

  /**
   * Starts the thread. The folder is read first: when its files run something by themselves the notice is shown and
   * the next send starts (OD-THR21: project settings are honoured, with a notice).
   */
  async #start(prompt: string, call: (action: Action) => Promise<CallResult>, started: (thread: string) => void): Promise<Outcome> {
    const folder = this.scratch ? '' : this.folder;
    if (folder === null) return { ok: false, words: NO_FOLDER };
    // A scratch folder is new and empty: it has no project files to read.
    if (folder !== '' && this.notice?.folder !== folder) {
      const read = await call({ call: 'folder', folder });
      const lines = !read.ok ? [NOT_CHECKED] : read.outcome.outcome === 'folder' ? read.outcome.notice : [];
      this.notice = { folder, lines };
      if (lines.length > 0) return { ok: false, words: SEND_AGAIN };
    }
    const { program, model, effort, mode } = this.choice;
    const result = await call({ call: 'start', program, folder, mode, model, effort, prompt });
    if (!result.ok) {
      // The files can change before the next send: it reads them again.
      this.notice = null;
      return { ok: false, words: result.words };
    }
    if (result.outcome.outcome !== 'started') return { ok: false, words: NOT_STARTED };
    started(result.outcome.thread);
    return { ok: true };
  }
}
