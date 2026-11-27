<script lang="ts">
  import { page } from '$app/state';
  import EmptyState from '$lib/states/EmptyState.svelte';
  import FormButton from '$lib/forms/FormButton.svelte';
  import MobileCode from '$lib/code/mobile/MobileCode.svelte';
  import { isMobile } from '$lib/platform/mobile';
  import CodePage from '$lib/code/chat/CodePage.svelte';
  import { keepFresh, threadsStore } from '$lib/agent-threads/watch.svelte';
  import { capabilities } from '$lib/platform/platform';
  import NodeCode from '$lib/code/nodes/NodeCode.svelte';

  const client = capabilities();
  const here = client.agentThreads;
  const mobile = isMobile(navigator);
  const phone = client.deviceLink && mobile;
  const store = threadsStore();
  let remote = $state(!here && !mobile);
  $effect(() => (here ? keepFresh(store) : undefined));
</script>

{#if remote && !(phone && page.url.searchParams.has('thread'))}
  <NodeCode onlocal={here || mobile ? () => (remote = false) : undefined} returnLabel={phone ? 'Code threads' : 'Code on this computer'} />
{:else if here}
  <CodePage {store} onnodes={() => (remote = true)} />
{:else if phone}
  <MobileCode onnodes={() => (remote = true)} />
{:else}
  <EmptyState title="Open Code in Anima" line="Code currently shows threads started by Anima on this computer. Open the desktop app on that computer.">
    {#snippet action()}<FormButton onclick={() => (remote = true)}>Code on a node</FormButton>{/snippet}
  </EmptyState>
{/if}
