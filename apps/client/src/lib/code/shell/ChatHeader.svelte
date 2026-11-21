<script lang="ts">
  import type { Snippet } from 'svelte';
  import { folderName, type ThreadRow } from '$lib/agent-threads/thread';
  import Breadcrumb from './Breadcrumb.svelte';
  import TopBand from './TopBand.svelte';

  interface Props {
    /** The open thread; none when no thread is open. */
    thread?: ThreadRow | undefined;
    /** What stands at the start of the band, for example the toggle of the thread list while the list is hidden. */
    start?: Snippet;
    /** What stands at the end of the band, for example the buttons of the panels. */
    end?: Snippet;
    /** Starts a new thread in the folder of this thread. */
    onproject?: () => void;
    /** Opens the actions of the thread, at its title. */
    onactions?: (anchor: HTMLElement) => void;
  }

  const { thread, start, end, onproject, onactions }: Props = $props();
  const project = $derived(thread?.folder === null || thread === undefined ? undefined : folderName(thread.folder));
</script>

<!--
  The head of the chat column of Code: the top band with the path of the open thread (report 02, Section 6). It is the
  last band of its row until a right panel is there, so it keeps the room of the window controls of a window with no frame.
-->
<div class="head" class:empty={thread === undefined}>
  <TopBand clearsWindowControls {start} {end}>
    {#if thread !== undefined}
      <Breadcrumb {project} title={thread.title ?? 'Untitled thread'} {onproject} {onactions} />
    {/if}
  </TopBand>
</div>

<style>
  /* With no thread open the head has a line under it; in a thread it has none (report 02, Section 6.1). */
  .empty {
    border-block-end: var(--border-thin) solid var(--border-subtle);
  }
</style>
