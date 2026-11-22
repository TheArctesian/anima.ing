<script lang="ts">
  import type { CallResult } from '$lib/agent-threads/live/live.svelte';
  import type { RunRow } from '$lib/agent-threads/live/run';
  import Card from './Card.svelte';
  import ReadOnlyBar from './ReadOnlyBar.svelte';
  import { untrack } from 'svelte';
  import { phaseOf, runEngine, type FileSearch, type QueueCall } from './engine';
  import type { SetCall } from './run/choice';
  import { heldQueue, NOT_WATCHED, type Went } from './queue/held.svelte';
  import './composer.css';

  interface Props {
    /** The run of the chat; null for a chat that is only read, which no program of this computer runs. */
    row: RunRow | null;
    onSend: (text: string) => Promise<CallResult>;
    onInterrupt: () => Promise<CallResult>;
    /** Ends the program. The composer has no control for it (T3's Stop interrupts the turn): the chat's header has. */
    onStop: () => Promise<CallResult>;
    /** The calls of the runner's own queue for this chat (TH109); without them the window holds the queue. */
    onQueue?: QueueCall;
    /** The runner's `set` call for this chat (TH110): the mode and the effort after the start. */
    onSet?: SetCall;
    /** The runner's file list for this chat (`live.files`), for the `@` list; without it the list says there is no search. */
    onFiles?: FileSearch;
    /** The key under which the unsent text is kept: the chat's id. */
    draft?: string;
  }

  /*
   * The composer of a chat of the Code section (TC4; 03 Part B). It sets no width and no place: the chat column
   * gives it the lane and pins it to the foot.
   */
  const { row, onSend, onInterrupt, onQueue, onSet, onFiles, draft }: Props = $props();
  const chat = $derived(row?.id);
  // Looked up apart from the engine: the engine must follow the queue's messages, and state is not followed by the
  // computation that made it.
  const queue = $derived(chat === undefined || onQueue !== undefined ? null : heldQueue(chat));
  const engine = $derived(row === null ? null : runEngine(row, { onSend, onInterrupt, onQueue, onSet, onFiles }, queue));
  // A run whose state is not known says nothing of its turn: the queue is told only of states that are.
  const phase = $derived(row === null || row.state === 'unknown' ? null : phaseOf(row));

  // The queue of the chat is told of each turn's end while this composer shows the chat, and sends through it.
  $effect(() => {
    if (chat === undefined || phase === null || queue === null) return;
    const watched = queue;
    const mine = chat;
    const asking = row?.state === 'needs_you';
    // The page gives one `onSend` for whichever chat is open: a message of this queue never goes to a different chat.
    const send = async (prompt: string): Promise<Went> => {
      if (row?.id !== mine) return NOT_WATCHED;
      const result = await onSend(prompt);
      return result.ok ? { ok: true } : { ok: false, words: result.words };
    };
    untrack(() => {
      watched.attend(phase, send, asking);
    });
    return () => {
      watched.leave();
    };
  });
</script>

{#if engine === null}
  <ReadOnlyBar status="This chat is read from its file; no program of this computer runs it." />
{:else if engine.ended !== null}
  <ReadOnlyBar status={engine.ended} />
{:else}
  {#key draft ?? row?.id}<Card {engine} {draft} title={row?.title} />{/key}
{/if}
