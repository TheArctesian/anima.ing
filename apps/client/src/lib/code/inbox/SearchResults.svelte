<script lang="ts">
  import { shownTitle } from '$lib/agent-threads/thread';
  import CodeIcon from '../icons/CodeIcon.svelte';
  import { shortAge } from '../status/status';
  import type { InboxRow } from './inbox';

  interface Props {
    hits: readonly InboxRow[];
    now: Date;
    chosen: string | null;
    /** The row the arrows in the search field reach (02 section 5.10). */
    at: number;
    onOpen: (id: string) => void;
    settled: (id: string) => boolean;
  }

  const { hits, now, chosen, at, onOpen, settled }: Props = $props();
  let list = $state<HTMLElement>();

  // The highlight stays in sight as the arrows move it (09 finding 11).
  $effect(() => {
    list?.querySelectorAll('[role="option"]')[at]?.scrollIntoView({ block: 'nearest' });
  });
</script>

<!-- A search replaces the list (02 section 5.10): the arrows in the field move the highlight, Enter opens. -->
<!-- The count, said when it changes, as the palette does (09 finding 10). -->
<p class="hidden" role="status">{hits.length === 0 ? '' : `${String(hits.length)} ${hits.length === 1 ? 'thread' : 'threads'} found`}</p>
{#if hits.length === 0}
  <p class="none" role="status">No threads found</p>
{:else}
  <ul class="hits" id="inbox-search-results" role="listbox" aria-label="Search results" bind:this={list}>
    {#each hits as item, index (item.row.id)}
      <!-- The keys stay in the search field; a pointer opens the row. -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <li
        role="option"
        id="hit-{item.row.id}"
        aria-selected={index === at}
        aria-current={item.row.id === chosen ? 'true' : undefined}
        class:at={index === at}
        class:open={item.row.id === chosen}
        class:settled={settled(item.row.id)}
        data-thread={item.row.id}
        onclick={() => {
          onOpen(item.row.id);
        }}
      >
        <CodeIcon name="folder" />
        <span class="title">{shownTitle(item.row)}</span>
        <span class="age">{item.since === null ? '' : shortAge(item.since.getTime(), now.getTime())}</span>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .hits {
    display: flex;
    flex-direction: column;
    gap: var(--border-thin);
    margin: 0;
    padding: var(--space-2);
  }

  li {
    display: flex;
    gap: var(--code-row-pad-inline);
    align-items: center;
    min-block-size: var(--code-slim);
    padding: var(--space-1) var(--code-row-pad-inline);
    border-radius: var(--radius-md);
    color: var(--code-quiet-text);
    font-size: var(--code-text-body);
    list-style: none;
    cursor: pointer;
  }

  li:hover {
    background: var(--surface-hover);
  }

  li.at,
  li.open {
    background: var(--surface-selected);
    color: var(--text);
  }

  .title {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    font-weight: var(--weight-medium);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .settled .title {
    font-weight: var(--weight-regular);
  }

  .age {
    flex: none;
    min-inline-size: var(--space-8);
    font-size: var(--code-text-meta);
    font-variant-numeric: tabular-nums;
    text-align: end;
  }

  .hidden {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    margin: 0;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  .none {
    margin: 0;
    padding: var(--space-6) var(--space-2);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    text-align: center;
  }
</style>
