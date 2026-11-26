<script lang="ts">
  import CodePage from '$lib/code/chat/CodePage.svelte';
  import { keepFresh, threadsStore } from '$lib/agent-threads/watch.svelte';
  import { capabilities } from '$lib/platform/platform';
  import NodeCode from '$lib/code/nodes/NodeCode.svelte';

  const client = capabilities();
  const here = client.agentThreads;
  const store = threadsStore();
  let remote = $state(!here);
  $effect(() => (here ? keepFresh(store) : undefined));
</script>

{#if remote}
  <NodeCode onlocal={here ? () => (remote = false) : undefined} />
{:else}
  <CodePage {store} onnodes={() => (remote = true)} />
{/if}
