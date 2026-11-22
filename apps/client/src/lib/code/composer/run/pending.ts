/**
 * The most that the runner holds of files that wait for their prompt, of all chats together (`MOST_PENDING` and
 * `MOST_PENDING_BYTES` of its `runner/attachments.rs`). The runner checks it again: a different window can hold files too.
 */
export const PENDING = { count: 200, bytes: 1024 * 1024 * 1024 } as const;

export const PENDING_FULL =
  'The files that wait to be sent, of all chats together, can be 200 files and 1 GB. Send or remove the files of a different chat, then attach again.';

/** Which of the incoming files the runner still has room for, in order, beside the ones that wait already; `over`: how many it has not. */
export function roomFor<F>(waiting: readonly { readonly size: number }[], incoming: readonly F[], size: (file: F) => number): { fits: F[]; over: number } {
  let count = waiting.length;
  let bytes = waiting.reduce((sum, held) => sum + held.size, 0);
  const fits: F[] = [];
  for (const file of incoming) {
    if (count >= PENDING.count || bytes + size(file) > PENDING.bytes) continue;
    fits.push(file);
    count += 1;
    bytes += size(file);
  }
  return { fits, over: incoming.length - fits.length };
}
