import { admit, attachmentOf, type Attachment } from './attachments/attach';
import type { ComposerEngine, Message, Outcome } from './engine';
import { sendFace, stopAfter, STOP_LANDED_MS, STOP_PENDING_MS, type FollowUp, type SendFace, type StopPhase, type Turn } from './primary/send';

/** The unsent text of each chat, by the key its page gives: it outlives the composer while the window is open. */
// eslint-disable-next-line svelte/prefer-svelte-reactivity -- read once when a composer is made, never by an effect
const kept = new Map<string, { readonly prompt: string; readonly attachments: readonly Attachment[] }>();

const STILL_READING = 'An attachment is still being read.';
const FAILED_FILE = 'Remove the attachment that failed, then send.';

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
  followUp: FollowUp = 'steer';

  #engine: () => ComposerEngine;
  #key: string | undefined;
  #aside: Message | null = null;
  #timer: ReturnType<typeof setTimeout> | undefined;
  #serial = 0;

  constructor(engine: () => ComposerEngine, key: string | undefined, modifier: string) {
    this.#engine = engine;
    this.#key = key;
    this.modifier = modifier;
    const draft = key === undefined ? undefined : kept.get(key);
    if (draft !== undefined) {
      this.prompt = draft.prompt;
      this.attachments = draft.attachments;
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
    if (went) return;
    const typed = this.prompt === (editing === null ? '' : (aside?.prompt ?? '')) ? '' : this.prompt;
    this.#fill({ prompt: typed === '' ? message.prompt : `${message.prompt}\n${typed}`, attachments: message.attachments });
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
    if (this.#aside !== null && (this.#aside.prompt !== '' || this.#aside.attachments.length > 0)) this.#fill(this.#aside);
    this.#endEdit();
    this.keep();
  }

  /** The composer goes away: no timer of it fires after. */
  dispose(): void {
    clearTimeout(this.#timer);
  }

  add(files: readonly File[]): void {
    const { taken, refused } = admit(this.attachments, files);
    const made = taken.map((entry) => {
      this.#serial += 1;
      const preview = entry.kind === 'image' || entry.kind === 'video' ? URL.createObjectURL(entry.file) : null;
      return attachmentOf(entry, `a${String(this.#serial)}`, preview);
    });
    this.attachments = [...this.attachments, ...made];
    this.problem = refused.at(-1)?.words ?? null;
  }

  remove(id: string): void {
    const gone = this.attachments.find((attachment) => attachment.id === id);
    if (gone?.preview?.startsWith('blob:') === true) URL.revokeObjectURL(gone.preview);
    this.attachments = this.attachments.filter((attachment) => attachment.id !== id);
  }

  /** Opens a queued message in the composer; the chat's own unsent text is set aside (03 3.6). */
  edit(id: string): void {
    const message = this.#engine().queued.find((queued) => queued.id === id);
    if (message === undefined) return;
    if (this.editing === null) this.#aside = { prompt: this.prompt, attachments: this.attachments };
    this.editing = id;
    this.prompt = message.prompt;
    this.attachments = [];
  }

  cancelEdit(): void {
    if (this.editing === null) return;
    this.#fill(this.#aside);
    this.#endEdit();
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
    this.attachments = message?.attachments ?? [];
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
