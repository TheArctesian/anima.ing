<script lang="ts">
  import CodePage from '$lib/code/chat/CodePage.svelte';
  import { keepFresh, threadsStore } from '$lib/agent-threads/watch.svelte';
  import { capabilities } from '$lib/platform/platform';
  import EmptyState from '$lib/states/EmptyState.svelte';

  const here = capabilities().agentThreads;
  const store = threadsStore();
  $effect(() => (here ? keepFresh(store) : undefined));
</script>

<!-- The page Code (THR-011, TC2): the copy of T3 Code's shell and inbox. It reads the session files of this computer
     through the desktop shell; it sends nothing to the hub. -->
{#if here}<CodePage {store} />{:else}<EmptyState title="Anima Code is in the desktop application" line="This page reads the sessions of Claude Code and Codex on the computer where the application runs." />{/if}
