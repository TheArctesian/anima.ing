<script lang="ts">
  import CodeIcon from '../icons/CodeIcon.svelte';
  import ScrollArea from '../kit/ScrollArea.svelte';
  import Shine from '../status/Shine.svelte';
  import Stamp from './meta/Stamp.svelte';
  import type { WorkEntry } from './turns';
  import { entryLabel, groupIcon, groupLabel, WORK_ICONS } from './words';
  import WorkRow from './WorkRow.svelte';

  interface Props {
    /** The tool calls and thoughts that follow each other in one turn. */
    entries: readonly WorkEntry[];
    /** The group is the newest part of a running turn: it shows as one live line (A.8.4). */
    live?: boolean;
    now: Date;
  }

  const { entries, live = false, now }: Props = $props();
  let open = $state(false);
  const newest = $derived(entries.at(-1));
  const label = $derived(newest === undefined ? '' : live ? entryLabel(newest) : groupLabel(entries));
  /** The name of the line, as T3 Code's group header: its words and a failure, never the time shown on hover. */
  const name = $derived(entries.some((entry) => entry.tool?.failed === true) ? `${label}, tool call failed` : label);
  const id = $props.id();
</script>

<!--
  A group of the work log (03 section A.8.4). One entry shows as itself. While the turn runs, the group is one live line:
  its newest entry, which opens the whole group. After it, one line with the group's sentence. Open, a scrolling list.
  Neither line has a chevron, as in T3 Code (fidelity 2). While its newest tool has not failed, T3 Code's shine crosses
  the live line where motion may run (operator, 2026-11-23).
-->
{#if entries.length === 1 && newest !== undefined && !live}
  <WorkRow entry={newest} {now} />
{:else if newest !== undefined}
  <div class="group">
    <button type="button" class="line" class:live aria-label={name} aria-expanded={open} aria-controls={open ? `${id}-list` : undefined} onclick={() => (open = !open)}>
      <span class="slot"><CodeIcon name={live ? WORK_ICONS[newest.kind] : groupIcon(entries)} /></span>
      <span class="label">{#if live && newest.tool?.failed !== true}<Shine text={label} />{:else}{label}{/if}</span>
      {#if !live && newest.at !== null}<span class="when"><Stamp at={newest.at} {now} /></span>{/if}
    </button>
    {#if open}
      <div class="list" id="{id}-list">
        <ScrollArea label="Tool calls">
          {#each entries as entry (entry.key)}<WorkRow {entry} {now} inner />{/each}
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

  /* Fidelity 20: an open header has no space under it; its content 4 px. */
  /* Fidelity 5: the time of a finished group, on hover or focus only. */
  .when {
    display: inline-flex;
    flex: none;
    opacity: 0;
    transition: opacity var(--spring-snappy);
  }

  .line:hover .when,
  .line:focus-visible .when {
    opacity: 1;
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
