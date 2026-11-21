<script lang="ts">
  import type { Answer } from '$lib/agent-threads/live/run';
  import { threadState } from '$lib/agent-threads/state';
  import type { ThreadsStore } from '$lib/agent-threads/store.svelte';
  import LiveView from '$lib/agent-threads/view/LiveView.svelte';
  import ThreadView from '$lib/agent-threads/view/ThreadView.svelte';
  import EmptyState from '$lib/states/EmptyState.svelte';
  import ErrorState from '$lib/states/ErrorState.svelte';
  import LoadingState from '$lib/states/LoadingState.svelte';

  interface Props {
    store: ThreadsStore;
    now: Date;
    toolsOpen: boolean;
  }

  const { store, now, toolsOpen }: Props = $props();
  const live = $derived(store.live);
</script>

<!-- The open thread as the agent-threads views draw it today; stage 3 puts the timeline of the copy here. -->
<div class="body" aria-busy={store.loadingThread}>
  {#if store.threadProblem !== null}
    <ErrorState message={store.threadProblem} onRetry={() => (store.chosen === null ? undefined : void store.open(store.chosen))} />
  {:else if store.liveThread !== null && live !== null}
    {@const id = store.liveThread.row.id}
    <!-- Keyed by the thread, so that a draft of the composer never goes to a different thread. -->
    {#key id}
      <LiveView
        thread={store.liveThread}
        {toolsOpen}
        onSend={(text) => live.call({ call: 'send', thread: id, text })}
        onAnswer={(request: string, answer: Answer) => live.call({ call: 'answer', thread: id, request, answer })}
        onInterrupt={() => live.call({ call: 'interrupt', thread: id })}
        onStop={() => live.call({ call: 'stop', thread: id })}
      />
    {/key}
  {:else if store.thread !== null && store.thread.id === store.chosen}
    <ThreadView thread={store.thread} {toolsOpen} state={threadState(store.thread, now, store.seen)} />
  {:else if store.chosen !== null}
    <LoadingState label="Loading the thread" rows={8} />
  {:else}
    <EmptyState title="No thread is open" line="Select a thread in the list, or press the Down key." />
  {/if}
</div>

<style>
  .body {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-block-size: 0;
  }
</style>
