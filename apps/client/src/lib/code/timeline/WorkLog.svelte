<script lang="ts">
  import CodeIcon from '../icons/CodeIcon.svelte';
  import ScrollArea from '../kit/ScrollArea.svelte';
  import type { WorkEntry } from './turns';
  import { entryLabel, groupIcon, groupLabel, WORK_ICONS } from './words';
  import WorkRow from './WorkRow.svelte';

  interface Props {
    /** The tool calls and thoughts that follow each other in one turn. */
    entries: readonly WorkEntry[];
    /** The group is the newest part of a running turn: it shows as one live line (A.8.4). */
    live?: boolean;
  }

  const { entries, live = false }: Props = $props();
  let open = $state(false);
  const newest = $derived(entries.at(-1));
  const id = $props.id();
</script>

<!--
  A group of the work log (03 section A.8.4). One entry shows as itself. While the turn runs, the group is one live line:
  its newest entry, which opens the whole group. After it, one line with the group's sentence. Open, a scrolling list.
-->
{#if entries.length === 1 && newest !== undefined && !live}
  <WorkRow entry={newest} />
{:else if newest !== undefined}
  <div class="group">
    <button type="button" class="line" class:live aria-expanded={open} aria-controls={open ? `${id}-list` : undefined} onclick={() => (open = !open)}>
      <span class="slot"><CodeIcon name={live ? WORK_ICONS[newest.kind] : groupIcon(entries)} /></span>
      <span class="label">{live ? entryLabel(newest) : groupLabel(entries)}</span>
      <span class="chevron" class:open><CodeIcon name="chevron-right" /></span>
    </button>
    {#if open}
      <div class="list" id="{id}-list">
        <ScrollArea label="Tool calls">
          {#each entries as entry (entry.key)}<WorkRow {entry} inner />{/each}
        </ScrollArea>
      </div>
    {/if}
  </div>
{/if}

<style>
  .line {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    inline-size: 100%;
    min-block-size: var(--code-timeline-line);
    padding: 2px;
    border: none;
    border-radius: var(--radius-md);
    background: none;
    color: var(--code-quiet-text);
    font: inherit;
    font-size: var(--code-text-body);
    text-align: start;
    cursor: pointer;
  }

  .line.live {
    color: var(--text);
  }

  .line:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: -2px;
  }

  .slot {
    display: inline-flex;
    flex: none;
    justify-content: center;
    inline-size: var(--code-timeline-icon-slot);
  }

  .label {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chevron {
    display: inline-flex;
    flex: none;
    /* A spring token: no motion under the reduced setting of the system or of Settings. */
    transition: transform var(--spring-snappy);
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .list {
    display: grid;
    max-block-size: var(--code-timeline-group-max);
    padding-block-end: var(--space-1);
  }

  .list > :global(*) {
    min-block-size: 0;
    scroll-padding-block: var(--code-timeline-line);
    scrollbar-gutter: stable;
  }
</style>
