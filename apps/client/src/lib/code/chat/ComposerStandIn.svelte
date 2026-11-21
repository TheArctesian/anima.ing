<script lang="ts">
  import type { LiveThreads } from '$lib/agent-threads/live/live.svelte';
  import type { LiveThread } from '$lib/agent-threads/live/timeline';
  import { folderName, programName } from '$lib/agent-threads/thread';
  import AskCard from '$lib/agent-threads/view/AskCard.svelte';
  import Composer from '$lib/agent-threads/view/Composer.svelte';

  interface Props {
    thread: LiveThread;
    live: LiveThreads;
  }

  const { thread, live }: Props = $props();
  const id = $derived(thread.row.id);
  const questions = $derived(thread.asks.filter((open) => open.ask.kind === 'question'));
</script>

<!--
  The foot of the chat column until the composer of stage 4 lands: today's agent-threads composer, and over it the form
  of each question the program asks (the timeline shows the question as a line; stage 4 docks its form on the composer).
-->
{#key id}
  <div class="foot">
    {#each questions as open (open.request)}
      <AskCard {open} program={programName(thread.row.program)} folder={folderName(thread.row.folder)} onAnswer={(request, answer) => live.call({ call: 'answer', thread: id, request, answer })} />
    {/each}
    <Composer
      row={thread.row}
      onSend={(text) => live.call({ call: 'send', thread: id, text })}
      onInterrupt={() => live.call({ call: 'interrupt', thread: id })}
      onStop={() => live.call({ call: 'stop', thread: id })}
    />
  </div>
{/key}

<style>
  .foot {
    display: grid;
    gap: var(--space-3);
  }
</style>
