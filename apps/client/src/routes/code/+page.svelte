<script lang="ts">
  import CodePage from '$lib/code/chat/CodePage.svelte';
  import { keepFresh, threadsStore } from '$lib/agent-threads/watch.svelte';
  import { capabilities } from '$lib/platform/platform';
  import EmptyState from '$lib/states/EmptyState.svelte';

  const client = capabilities();
  const here = client.agentThreads;
  const store = threadsStore();
  $effect(() => (here ? keepFresh(store) : undefined));
</script>

{#if here}
  <CodePage {store} />
{:else}
  <EmptyState title="Open Code in Anima" line="Code currently shows threads started by Anima on this computer. Open the desktop app on that computer." />
{/if}
