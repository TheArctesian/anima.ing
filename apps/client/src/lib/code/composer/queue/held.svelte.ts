import type { Queued } from './queue';

/** How a message of the queue went, in the words of a failure. */
export type Went = { readonly ok: true } | { readonly ok: false; readonly words: string };

/** Sends one prompt to the chat of a queue as a turn of its own, or into the running one. */
export type Sender = (prompt: string) => Promise<Went>;

export type QueuePhase = 'idle' | 'running' | 'ended';

/** A queue holds no more: a person who queues twenty messages has lost track of what the program will be told. */
export const QUEUE_MOST = 20;

const OK: Went = { ok: true };
const FULL = `The queue holds ${String(QUEUE_MOST)} messages. Remove one, or wait for the turn to end.`;
const GONE = 'That message is no longer queued.';
const FAILED = 'The message could not be sent.';

/**
 * The answer of a sender whose chat is no longer the one on screen: the message did not go and nothing is wrong. The
 * queue keeps it due and tries again when the chat is watched.
 */
export const NOT_WATCHED: Went = { ok: false, words: 'The chat of this message is not open.' };

/**
 * The messages of one chat that wait for its turn to end, held by this window (03 "3. Queued messages"). The
 * runner keeps no queue, so this one does what the runner would: when a turn ends, the first message goes as the
 * next turn. It is lost when the app closes, and it sends only while something watches the chat (`attend`).
 *
 * It sends nothing on its own after a stop by the person, after a message that did not go, and while its first
 * message is open in the composer: a message goes late or not at all, never changed or out of order.
 */
export class HeldQueue {
  messages = $state<readonly Queued[]>([]);
  /** Why nothing goes on its own, when nothing does: shown under the messages. */
  waits = $state<string | null>(null);

  #was: QueuePhase | null = null;
  /** A turn ended and the message that was to go then has not gone yet. */
  #due = false;
  #open: string | null = null;
  #send: Sender | null = null;
  #serial = 0;

  add(prompt: string): Went {
    if (this.messages.length >= QUEUE_MOST) return { ok: false, words: FULL };
    this.#serial += 1;
    this.messages = [...this.messages, { id: `q${String(this.#serial)}`, prompt, saving: false }];
    return OK;
  }

  change(id: string, prompt: string): Went {
    if (!this.messages.some((message) => message.id === id)) return { ok: false, words: GONE };
    this.messages = this.messages.map((message) => (message.id === id ? { ...message, prompt } : message));
    return OK;
  }

  remove(id: string): Went {
    if (!this.messages.some((message) => message.id === id)) return { ok: false, words: GONE };
    this.#drop(id);
    return OK;
  }

  /** The person stopped the turn: what waits behind it is theirs to start. */
  hold(why: string): void {
    if (this.messages.length > 0) this.waits = why;
  }

  /** The person sent something themself: the queue goes on after that turn. */
  release(): void {
    this.waits = null;
  }

  /** The queued message that is open in the composer, or null when none is: it is not sent while it is changed. */
  opened(id: string | null): void {
    this.#open = id;
    if (id === null) void this.#advance();
  }

  /** "Send now": this message goes at once, whatever the queue waits for, and the rest go on after its turn. */
  async sendNow(id: string, send: Sender): Promise<Went> {
    const message = this.messages.find((queued) => queued.id === id);
    if (message === undefined) return { ok: false, words: GONE };
    const went = await this.#go(message, send);
    if (went.ok) this.release();
    return went;
  }

  /**
   * Tells the queue the phase of its chat, and how to send to it for as long as the caller watches. A turn that
   * ended sends the first message.
   */
  attend(phase: QueuePhase, send: Sender): void {
    this.#send = send;
    const ended = this.#was === 'running' && phase === 'idle';
    this.#was = phase;
    if (phase === 'running') this.#due = false;
    if (ended) this.#due = true;
    // Also when the turn ended while nobody watched: what was due then goes now.
    if (phase === 'idle') void this.#advance();
  }

  /** Nobody watches the chat now: nothing goes until `attend` is called again. */
  leave(): void {
    this.#send = null;
  }

  get empty(): boolean {
    return this.messages.length === 0;
  }

  async #advance(): Promise<void> {
    const first = this.messages[0];
    if (!this.#due || this.waits !== null || first === undefined || first.id === this.#open || this.#send === null) return;
    this.#due = false;
    const went = await this.#go(first, this.#send);
    if (went === NOT_WATCHED) this.#due = true;
    else if (!went.ok) this.waits = `The queue stopped: ${went.words}`;
  }

  /** Takes the message out and sends it; one that did not go comes back where it was. */
  async #go(message: Queued, send: Sender): Promise<Went> {
    const at = this.messages.indexOf(message);
    this.#drop(message.id);
    const went = await send(message.prompt).catch((): Went => ({ ok: false, words: FAILED }));
    if (went.ok) return went;
    const back = [...this.messages];
    back.splice(Math.min(at, back.length), 0, message);
    this.messages = back;
    return went;
  }

  #drop(id: string): void {
    this.messages = this.messages.filter((message) => message.id !== id);
    if (this.messages.length === 0) this.waits = null;
  }
}

// eslint-disable-next-line svelte/prefer-svelte-reactivity -- looked up by key when an engine is made, never read by an effect
const queues = new Map<string, HeldQueue>();

/** The queue of a chat, by the chat's id: one for as long as the window is open. */
export function heldQueue(chat: string): HeldQueue {
  const known = queues.get(chat);
  if (known !== undefined) return known;
  const made = new HeldQueue();
  queues.set(chat, made);
  return made;
}

/**
 * For the page that sees every chat: tells each queue that holds a message the phase of its chat, so a queued
 * message goes when its turn ends while a different chat is open. A chat with nothing queued is left alone.
 */
export function attendAll(chats: readonly { readonly id: string; readonly phase: QueuePhase }[], send: (chat: string, prompt: string) => Promise<Went>): void {
  for (const chat of chats) {
    const queue = queues.get(chat.id);
    if (queue === undefined) continue;
    if (chat.phase === 'ended' && queue.empty) queues.delete(chat.id);
    else queue.attend(chat.phase, (prompt) => send(chat.id, prompt));
  }
}
