<script lang="ts">
  import { threadState } from '$lib/agent-threads/state';
  import type { ThreadsStore } from '$lib/agent-threads/store.svelte';
  import ThreadView from '$lib/agent-threads/view/ThreadView.svelte';
  import EmptyState from '$lib/states/EmptyState.svelte';
  import ErrorState from '$lib/states/ErrorState.svelte';
  import LoadingState from '$lib/states/LoadingState.svelte';
  import Timeline from '../timeline/Timeline.svelte';
  import { decided } from './decide';

  interface Props {
    store: ThreadsStore;
    now: Date;
    toolsOpen: boolean;
  }

  const { store, now, toolsOpen }: Props = $props();
  const live = $derived(store.live);

  function decide(id: string, request: string, decision: string): void {
    const open = store.liveThread?.asks.find((each) => each.request === request);
    if (live === null || open === undefined) return;
    const { answer, interrupt } = decided(decision, open.ask);
    void live.call({ call: 'answer', thread: id, request, answer }).then((result) => (result.ok && interrupt ? live.call({ call: 'interrupt', thread: id }) : result));
  }
</script>

<!-- The open thread: the timeline of the copy (stage 3) for a thread the runner drives, today's view for a stored one. -->
<div class="body" aria-busy={store.loadingThread}>
  {#if store.threadProblem !== null}
    <ErrorState message={store.threadProblem} onRetry={() => (store.chosen === null ? undefined : void store.open(store.chosen))} />
  {:else if store.liveThread !== null && live !== null}
    {@const thread = store.liveThread}
    {#key thread.row.id}
      <Timeline
        entries={thread.entries}
        streaming={thread.streaming}
        asks={thread.asks}
        working={thread.row.state === 'working' || thread.row.state === 'starting'}
        dropped={thread.dropped}
        onDecide={(request, decision) => {
          decide(thread.row.id, request, decision);
        }}
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
