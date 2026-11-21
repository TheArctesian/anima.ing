<script lang="ts">
  import type { Snippet } from 'svelte';
  import { folderName, type ThreadRow } from '$lib/agent-threads/thread';
  import Band from '../kit/Band.svelte';

  interface Props {
    /** The open thread; none when no thread is open. */
    thread?: ThreadRow | undefined;
    start?: Snippet;
    end?: Snippet;
  }

  const { thread, start, end }: Props = $props();
</script>

<!--
  A stand-in for the chat band of the shell session (`$lib/code/shell/ChatHeader.svelte`, branch code/shell-window),
  with the same props. When that branch lands, the Code page imports it in place of this file and this file goes.
-->
<Band ruled={thread === undefined} {start} {end}>
  {#if thread !== undefined}
    <span class="path">
      {#if thread.folder !== null}<span class="project">{folderName(thread.folder)}</span><span class="cut" aria-hidden="true">/</span>{/if}
      <span class="title">{thread.title ?? 'Untitled thread'}</span>
    </span>
  {/if}
</Band>

<style>
  .path {
    display: flex;
    gap: var(--space-2);
    align-items: center;
    min-inline-size: 0;
  }

  .project,
  .cut {
    flex: none;
    color: var(--code-quiet-text);
  }

  .title {
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
