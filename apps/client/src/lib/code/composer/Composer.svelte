<script lang="ts">
  import type { CallResult } from '$lib/agent-threads/live/live.svelte';
  import type { RunRow } from '$lib/agent-threads/live/run';
  import Card from './Card.svelte';
  import ReadOnlyBar from './ReadOnlyBar.svelte';
  import { runEngine } from './engine';
  import './composer.css';

  interface Props {
    /** The run of the chat; null for a chat that is only read, which no program of this computer runs. */
    row: RunRow | null;
    onSend: (text: string) => Promise<CallResult>;
    onInterrupt: () => Promise<CallResult>;
    /** Ends the program. The composer has no control for it (T3's Stop interrupts the turn): the chat's header has. */
    onStop: () => Promise<CallResult>;
    /** The key under which the unsent text is kept: the chat's id. */
    draft?: string;
  }

  /*
   * The composer of a chat of the Code section (TC4; 03 Part B). It sets no width and no place: the chat column
   * gives it the lane and pins it to the foot.
   */
  const { row, onSend, onInterrupt, draft }: Props = $props();
  const engine = $derived(row === null ? null : runEngine(row, { onSend, onInterrupt }));
</script>

{#if engine === null}
  <ReadOnlyBar status="This chat is read from its file; no program of this computer runs it." />
{:else if engine.ended !== null}
  <ReadOnlyBar status={engine.ended} />
{:else}
  {#key draft}<Card {engine} {draft} title={row?.title} />{/key}
{/if}
