<script lang="ts">
  import type { Snippet } from 'svelte';
  import { folderName, shownTitle, type ThreadRow } from '$lib/agent-threads/thread';
  import type { Anchor } from '$lib/overlay/helptip/placement';
  import Breadcrumb from './Breadcrumb.svelte';
  import TopBand from './TopBand.svelte';

  interface Props {
    /** The open thread; none when no thread is open. */
    thread?: ThreadRow | undefined;
    /** What stands at the end of the band, for example the buttons of the panels. */
    end?: Snippet;
    /** The thread list is hidden: the path starts after the list's toggle, 52 px in, as in T3 Code. */
    inset?: boolean;
    /** Starts a new thread in the folder of this thread. */
    onproject?: () => void;
    /** Opens the actions of the thread at `anchor`: under the title, or at the pointer of a right-click. */
    onactions?: (anchor: Anchor) => void;
    onrename?: (title: string) => Promise<string | null>;
    renaming?: boolean;
    onrenaming?: (on: boolean) => void;
  }

  const { thread, end, inset = false, onproject, onactions, onrename, renaming = false, onrenaming }: Props = $props();
  const project = $derived(thread?.folder === null || thread === undefined ? undefined : folderName(thread.folder));

  /** A right-click anywhere on the head opens the thread's menu at the pointer (T3 Code). */
  function oncontextmenu(event: MouseEvent): void {
    if (thread === undefined || renaming || onactions === undefined) return;
    event.preventDefault();
    onactions({ left: event.clientX, top: event.clientY, width: 0, height: 0 });
  }
</script>

<!--
  The head of the chat column of Code: the top band with the path of the open thread (report 02, Section 6). It is the
  last band of its row until a right panel is there, so it keeps the room of the window controls of a window with no
  frame, and T3's 96 px on the right for the panel toggles.
-->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="head" class:empty={thread === undefined} class:inset {oncontextmenu}>
  <TopBand clearsWindowControls keepsPanelRoom={end === undefined} {end}>
    {#if thread !== undefined}
      <Breadcrumb
        {project}
        title={shownTitle(thread)}
        {onproject}
        onactions={onactions === undefined
          ? undefined
          : (title) => {
              const box = title.getBoundingClientRect();
              onactions({ left: box.left, top: box.bottom, width: 0, height: 0 });
            }}
        {onrename}
        {renaming}
        {onrenaming}
      />
    {:else}
      <p class="none">No active thread</p>
    {/if}
  </TopBand>
</div>

<style>
  /* With no thread open the head has a line under it; in a thread it has none (report 02, Section 6.1). */
  .empty {
    border-block-end: var(--border-thin) solid var(--border-subtle);
  }

  /* 12 + 28 + 12 px: the toggle of the list stands in the corner, so the path starts after it (T3 Code). */
  .inset :global([data-code-band]) {
    padding-inline-start: calc(var(--space-3) * 2 + var(--control-sm));
  }

  .head :global([data-code-band]) {
    transition: padding-inline-start var(--spring-default);
  }

  .none {
    margin: 0;
    color: var(--code-quiet-text);
    font-size: var(--text-xs);
  }
</style>
