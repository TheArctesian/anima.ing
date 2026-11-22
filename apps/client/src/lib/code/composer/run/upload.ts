import { MOST_PART_BYTES, type AttachAction } from '$lib/agent-threads/live/attachments';
import type { CallResult } from '$lib/agent-threads/live/live.svelte';
import { shownName } from './shown-name';

type WithoutThread<A> = A extends unknown ? Omit<A, 'thread'> : never;

/** A call of the runner for a file of a prompt (TH111), without the thread: the page adds it. */
export type AttachAct = WithoutThread<AttachAction>;
export type AttachCall = (act: AttachAct) => Promise<CallResult>;
/** The same call as the page gives it: the composer names the chat of the file, so that a part or an `unattach` goes to that chat also when a different one is open by then. */
export type AttachTo = (act: AttachAct, thread: string) => Promise<CallResult>;

/** What `upload` reads of a `File`: its name, its size and its bytes. */
export interface Sendable {
  readonly name: string;
  readonly size: number;
  readonly slice: (start: number, end: number) => { arrayBuffer: () => Promise<ArrayBuffer> };
}

/** Who follows a file while it goes: `told` at each part that the runner has, `gone` when the person removed it. */
export interface UploadWatch {
  readonly told: (sent: number) => void;
  readonly gone: () => boolean;
}

/** How a file went: the runner has it under `id`, the person removed it, or it did not go, with the words to show. */
export type Uploaded = { readonly state: 'sent'; readonly id: string } | { readonly state: 'dropped' } | { readonly state: 'failed'; readonly words: string };

const DROPPED: Uploaded = { state: 'dropped' };
const OUT_OF_STEP = 'The runner did not take the file whole. Remove it and attach it again.';
const NOT_READ = (name: string): string => `'${name}' is empty or could not be read.`;

/* In pieces: the arguments of one call have a limit that a part of 2 MiB is over. */
const PIECE = 0x8000;

/** Standard base64 of bytes, as the runner reads `bytes`. */
export function base64(bytes: Uint8Array): string {
  let binary = '';
  for (let at = 0; at < bytes.length; at += PIECE) binary += String.fromCharCode(...bytes.subarray(at, at + PIECE));
  return btoa(binary);
}

async function part(file: Sendable, from: number): Promise<string | null> {
  try {
    return base64(new Uint8Array(await file.slice(from, Math.min(from + MOST_PART_BYTES, file.size)).arrayBuffer()));
  } catch {
    // A folder, or a file that was moved after it was chosen.
    return null;
  }
}

/**
 * Sends one file to the runner in parts of at most 2 MiB, in order: `attach` with the first part, `attach_more` with
 * each next one. It stops between two parts when the person removed the file. A file that the runner began to take
 * and that will not go is given up with `unattach`, so that it does not wait there.
 */
export async function upload(call: AttachCall, file: Sendable, mime: string, watch: UploadWatch): Promise<Uploaded> {
  let id: string | null = null;
  let sent = 0;
  const giveUp = (how: Uploaded): Uploaded => {
    if (id !== null) void call({ call: 'unattach', attachment: id }).catch(() => undefined);
    return how;
  };
  for (;;) {
    if (watch.gone()) return giveUp(DROPPED);
    const bytes = await part(file, sent);
    if (bytes === null) return giveUp({ state: 'failed', words: NOT_READ(shownName(file.name)) });
    const act: AttachAct = id === null ? { call: 'attach', name: file.name, mime, size: file.size, bytes } : { call: 'attach_more', attachment: id, bytes };
    const result: CallResult = await call(act).catch((): CallResult => ({ ok: false, words: 'The thread runner did not answer.' }));
    // A refusal of the runner names the file and says why.
    if (!result.ok) return giveUp({ state: 'failed', words: result.words });
    if (result.outcome.outcome !== 'attached') return giveUp({ state: 'failed', words: OUT_OF_STEP });
    // One file has one id: an answer that names a different one is not of this file.
    if (id !== null && result.outcome.id !== id) return giveUp({ state: 'failed', words: OUT_OF_STEP });
    id = result.outcome.id;
    sent = Math.min(sent + MOST_PART_BYTES, file.size);
    if (result.outcome.received !== sent) return giveUp({ state: 'failed', words: OUT_OF_STEP });
    watch.told(sent);
    if (sent >= file.size) return watch.gone() ? giveUp(DROPPED) : { state: 'sent', id };
  }
}
