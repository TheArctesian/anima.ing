import { admit, attachmentOf, REFUSAL, type Attachment, type AttachmentState } from './attachments/attach';
import { PENDING_FULL, roomFor } from './run/pending';
import type { ComposerEngine, Message, Outcome } from './engine';
import type { Choice } from './picker/catalogue';
import { shownName } from './run/shown-name';
import { sendFace, stopAfter, STOP_LANDED_MS, STOP_PENDING_MS, type FollowUp, type SendFace, type StopPhase, type Turn } from './primary/send';

/** The unsent text of each chat, by the key its page gives: it outlives the composer while the window is open. */
// eslint-disable-next-line svelte/prefer-svelte-reactivity -- read once when a composer is made, never by an effect
const kept = new Map<string, { readonly prompt: string; readonly attachments: readonly Attachment[] }>();

/** One count for every composer: a draft that is opened again keeps its files, and a new file must not take the id of one of them. */
let serial = 0;

const STILL_READING = 'An attachment is still being read.';
const FAILED_FILE = 'Remove the attachment that failed, then send.';

/** Frees the addresses of the previews: nothing draws them after the message went or the attachment was removed. */
function release(attachments: readonly Attachment[]): void {
  for (const attachment of attachments) if (attachment.preview?.startsWith('blob:') === true) URL.revokeObjectURL(attachment.preview);
}

/** Two choices that say the same model, effort and mode. */
function same(a: Choice, b: Choice | null): boolean {
  return b !== null && a.model === b.model && a.effort === b.effort && a.mode === b.mode;
}

/**
 * What one composer holds and does (03 B.7): the prompt, what is attached, the send that is on its way, the stop
 * that was asked for and the queued message that is open for a change. It calls the engine and nothing else.
 */
export class CardState {
  prompt = $state('');
  attachments = $state<readonly Attachment[]>([]);
  busy = $state(false);
  /** The words of the last failure, shown under the editor until the next try. */
  problem = $state<string | null>(null);
  stop = $state<StopPhase>('none');
  /** The id of the queued message that is open in the composer. */
  editing = $state<string | null>(null);
  /** ⌘ or Ctrl is held: the other follow-up (03 B.7.3). */
  alternate = $state(false);
  /** The choice the person made in the picker that the chat does not say yet. */
  asked = $state<Choice | null>(null);
  followUp: FollowUp = 'steer';

  #engine: () => ComposerEngine;
  #key: string | undefined;
  #aside: Message | null = null;
  #before: Choice | null = null;
  #timer: ReturnType<typeof setTimeout> | undefined;
  /** The files go to the runner one after the other: a part is 2 MiB, and a hundred files at one time would hold them all in memory. */
  #line: Promise<void> = Promise.resolve();
  #gone = false;

  constructor(engine: () => ComposerEngine, key: string | undefined, modifier: string) {
    this.#engine = engine;
    this.#key = key;
    this.modifier = modifier;
    const draft = key === undefined ? undefined : kept.get(key);
    if (draft !== undefined) {
      this.prompt = draft.prompt;
      // A file that was on its way when the composer went stopped there.
      release(draft.attachments.filter((attachment) => attachment.state.state === 'reading'));
      this.attachments = draft.attachments.filter((attachment) => attachment.state.state !== 'reading');
    }
  }

  readonly modifier: string;

  get running(): boolean {
    return this.#engine().phase === 'running';
  }

  get turn(): Turn {
    const engine = this.#engine();
    const states = this.attachments.map((attachment) => attachment.state.state);
    // With neither follow-up, nothing can go into a running turn: the button says why and does nothing.
    const noFollowUp = this.running && this.editing === null && !engine.steer.can && !engine.queue.can ? engine.steer.reason : null;
    const blocked = engine.ended ?? noFollowUp ?? (states.includes('failed') ? FAILED_FILE : states.includes('reading') ? STILL_READING : null);
    return {
      running: this.running,
      filled: this.prompt.trim() !== '' || this.attachments.length > 0,
      busy: this.busy,
      editing: this.editing !== null,
      followUp: this.followUp,
      alternate: this.alternate,
      steer: engine.steer,
      queue: engine.queue,
      blocked,
    };
  }

  get face(): SendFace {
    return sendFace(this.turn, this.modifier);
  }

  /** Keeps the unsent text for the next time this chat is opened; the text of a queued message is not the chat's. */
  keep(): void {
    if (this.#key === undefined || this.editing !== null) return;
    if (this.prompt === '' && this.attachments.length === 0) kept.delete(this.#key);
    else kept.set(this.#key, { prompt: this.prompt, attachments: this.attachments });
  }

  /**
   * The press of the round button, or Enter. A press in the half second after a stop landed does nothing. The editor
   * is emptied at once, so that what is typed while the message goes is the next message; a message that did not go
   * comes back in front of it.
   */
  async submit(alternate: boolean): Promise<void> {
    const face = sendFace({ ...this.turn, alternate }, this.modifier);
    if (face.disabled || this.busy || this.stop === 'landed') return;
    const engine = this.#engine();
    const message: Message = { prompt: this.prompt, attachments: this.attachments };
    const editing = this.editing;
    const aside = this.#aside;
    this.#endEdit();
    this.#fill(editing === null ? null : aside);
    const went = await this.#run(() => (face.action === 'update' && editing !== null ? engine.requeue(editing, message) : face.action === 'queue' ? engine.enqueue(message) : engine.send(message)));
    if (went) {
      // The change is saved: the queue may send the message again. A message that went needs its previews no more.
      if (editing !== null) engine.opened?.(null);
      else if (face.action !== 'queue') release(message.attachments);
      return;
    }
    const typed = this.prompt === (editing === null ? '' : (aside?.prompt ?? '')) ? '' : this.prompt;
    // What was attached while the message was on its way stays, after the files of the message.
    this.#fill({ prompt: typed === '' ? message.prompt : `${message.prompt}\n${typed}`, attachments: [...message.attachments, ...this.attachments] });
    if (editing === null) return;
    this.editing = editing;
    this.#aside = aside;
  }

  /** Stop: it waits for the turn to end for at most ten seconds, then can be pressed again (03 B.7.2). */
  async interrupt(): Promise<void> {
    if (this.stop === 'pending') return;
    this.stop = 'pending';
    this.#after(STOP_PENDING_MS, 'pending', 'timed_out');
    const outcome = await this.#engine().interrupt();
    if (outcome.ok) return;
    this.problem = outcome.words;
    this.stop = 'none';
  }

  /** The turn started or ended: a stop that was asked for has landed, or is forgotten. */
  turned(running: boolean): void {
    const next = stopAfter(this.stop, running);
    // The half second starts when the stop lands, not again at each update of the chat.
    if (next === 'landed' && this.stop !== 'landed') this.#after(STOP_LANDED_MS, 'landed', 'none');
    this.stop = next;
  }

  /**
   * The queued message that is open left the queue (it was sent, or removed elsewhere): its text stays as the chat's
   * unsent text when the chat had none, else the chat's own text comes back (03 3.6).
   */
  orphaned(): void {
    if (this.editing === null) return;
    if (this.#aside !== null && (this.#aside.prompt !== '' || this.#aside.attachments.length > 0)) {
      this.#giveUp(this.attachments);
      this.#fill(this.#aside);
    }
    this.#endEdit();
    this.#engine().opened?.(null);
    this.keep();
  }

  /** The composer goes away: no timer of it fires after, and the queue is told that a message was left open. */
  dispose(): void {
    clearTimeout(this.#timer);
    if (this.editing !== null) {
      this.#engine().abandoned?.();
      // The change of the queued message is lost, and with it the files that were attached to it.
      this.#giveUp(this.attachments);
      this.attachments = [];
    }
    // A file that is on its way stops at its next part: the page sends for the chat that is open, and that one changed.
    this.#gone = true;
    const unsent = this.attachments.filter((attachment) => attachment.state.state === 'reading');
    if (unsent.length === 0) return;
    release(unsent);
    this.attachments = this.attachments.filter((attachment) => !unsent.includes(attachment));
    this.keep();
  }

  /** Attaches the files that fit; `folders`: how many folders came with them, which are refused with words. */
  add(files: readonly File[], folders = 0): void {
    const admitted = admit(this.attachments, files);
    const sends = this.#engine().upload !== undefined;
    // The unsent files of the other chats wait at the runner too: what it has no room for is refused here, with words.
    const room = sends ? roomFor(this.#waiting(), admitted.taken, (entry) => entry.file.size) : { fits: admitted.taken, over: 0 };
    const taken = room.fits;
    const refused = room.over > 0 ? [...admitted.refused, { words: PENDING_FULL }] : admitted.refused;
    const made = taken.map((entry) => {
      serial += 1;
      const preview = entry.kind === 'image' || entry.kind === 'video' ? URL.createObjectURL(entry.file) : null;
      // The name that the program is told, not the one of the file: the two differ for a name with marks that hide.
      const made = attachmentOf(entry, `a${String(serial)}`, preview, sends ? { state: 'reading', sent: 0 } : { state: 'ready' });
      return { ...made, name: shownName(entry.file.name) };
    });
    this.attachments = [...this.attachments, ...made];
    this.problem = refused.at(-1)?.words ?? (folders > 0 ? REFUSAL.folders(folders) : null);
    if (!sends) return;
    made.forEach((attachment, at) => {
      const file = taken[at]?.file;
      // A file that throws does not stop the files after it.
      if (file !== undefined) this.#line = this.#line.then(() => this.#upload(attachment, file)).catch(() => undefined);
    });
  }

  /** The files of this window that the runner holds or is taking: of this composer, of the unsent text that is set aside, and of the other chats. */
  #waiting(): readonly Attachment[] {
    const others = [...kept].flatMap(([key, draft]) => (key === this.#key ? [] : draft.attachments));
    const mine = [...this.attachments, ...(this.#aside?.attachments ?? [])];
    return [...others.filter((attachment) => attachment.remote !== undefined), ...mine.filter((attachment) => attachment.remote !== undefined || attachment.state.state === 'reading')];
  }

  remove(id: string): void {
    const removed = this.attachments.filter((attachment) => attachment.id === id);
    // The reason of a file that did not go leaves with the file.
    if (removed.some((attachment) => attachment.state.state === 'failed' && attachment.state.words === this.problem)) this.problem = null;
    this.#giveUp(removed);
    this.attachments = this.attachments.filter((attachment) => attachment.id !== id);
  }

  /** Files that will not go: their previews are freed, and the runner is told of each that it holds, so that it does not wait there. */
  #giveUp(attachments: readonly Attachment[]): void {
    release(attachments);
    for (const attachment of attachments) if (attachment.remote !== undefined) this.#engine().discard?.(attachment.remote);
  }

  /** Gives one file to the engine and writes how it went on its attachment; a file that was removed meanwhile is given up by the engine. */
  async #upload(attachment: Attachment, file: File): Promise<void> {
    const held = (): boolean => !this.#gone && this.attachments.some((other) => other.id === attachment.id);
    const write = (state: AttachmentState, remote?: string): void => {
      if (held()) this.attachments = this.attachments.map((other) => (other.id === attachment.id ? { ...other, state, ...(remote === undefined ? {} : { remote }) } : other));
    };
    if (!held()) return;
    const engine = this.#engine();
    if (engine.upload === undefined) return;
    const went = await engine.upload(file, attachment.mime, { told: (sent) => write({ state: 'reading', sent }), gone: () => !held() }).catch((): { state: 'failed'; words: string } => ({ state: 'failed', words: REFUSAL.empty(attachment.name) }));
    if (went.state === 'sent' && !held()) engine.discard?.(went.id);
    else if (went.state === 'sent') write({ state: 'ready' }, went.id);
    if (went.state !== 'failed' || !held()) return;
    write({ state: 'failed', words: went.words });
    // Also under the editor: the tile of an image has only "Failed", and the reason is the runner's, as when it holds too many files.
    this.problem = went.words;
  }

  /** Opens a queued message in the composer; the chat's own unsent text is set aside (03 3.6). */
  edit(id: string): void {
    const message = this.#engine().queued.find((queued) => queued.id === id);
    if (message === undefined) return;
    if (this.editing === null) this.#aside = { prompt: this.prompt, attachments: this.attachments };
    // The files of the queued message that was open are of no message now: nothing draws them, and the runner must not hold them.
    else this.#giveUp(this.attachments);
    this.editing = id;
    this.#engine().opened?.(id);
    this.prompt = message.prompt;
    this.attachments = [];
  }

  cancelEdit(): void {
    if (this.editing === null) return;
    this.#giveUp(this.attachments);
    this.#fill(this.#aside);
    this.#endEdit();
    this.#engine().opened?.(null);
  }

  /** A new choice of the picker: the words of a refusal go under the editor, as those of a send do. */
  async choose(next: Choice): Promise<void> {
    const engine = this.#engine();
    const before = engine.choice;
    if (!(await this.#run(() => engine.choose(next)))) return;
    this.asked = next;
    this.#before = before;
  }

  /**
   * The choice the picker shows. A program can take a change only at its next turn, and the chat says the old one
   * until then: what the person asked for is shown until the chat's own choice moves.
   */
  shownChoice(now: Choice): Choice {
    return this.asked !== null && same(now, this.#before) ? this.asked : now;
  }

  /** The chat's own choice moved: the program has the change, or took a different one. */
  settled(now: Choice): void {
    if (this.asked !== null && !same(now, this.#before)) this.asked = null;
  }

  /** "Send now" and "Remove" of a queued message. */
  queued(id: string, act: 'send' | 'remove'): Promise<boolean> {
    const engine = this.#engine();
    return this.#run(() => (act === 'send' ? engine.sendQueued(id) : engine.unqueue(id)));
  }

  #endEdit(): void {
    this.editing = null;
    this.#aside = null;
  }

  #fill(message: Message | null): void {
    this.prompt = message?.prompt ?? '';
    // A file that was on its way when it was set aside stopped there: it does not come back as one that is read for ever.
    const stopped = (message?.attachments ?? []).filter((attachment) => attachment.state.state === 'reading');
    release(stopped);
    this.attachments = (message?.attachments ?? []).filter((attachment) => !stopped.includes(attachment));
    // Also when the composer is gone while a message went: the sent text must not come back as the unsent text.
    this.keep();
  }

  /** One call at a time; a call that throws is a failure with words, and never leaves the composer busy. */
  async #run(call: () => Promise<Outcome>): Promise<boolean> {
    if (this.busy) return false;
    this.busy = true;
    this.problem = null;
    let outcome: Outcome;
    try {
      outcome = await call();
    } catch {
      outcome = { ok: false, words: 'The message could not be sent.' };
    } finally {
      this.busy = false;
    }
    if (!outcome.ok) this.problem = outcome.words;
    return outcome.ok;
  }

  #after(ms: number, from: StopPhase, to: StopPhase): void {
    clearTimeout(this.#timer);
    this.#timer = setTimeout(() => {
      if (this.stop === from) this.stop = to;
    }, ms);
  }
}
