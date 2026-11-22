<script lang="ts">
  import type { Anchor } from '$lib/overlay/helptip/placement';
  import CodeIcon from '../icons/CodeIcon.svelte';
  import type { RowAction } from './actions';
  import { SHELF_TITLES, type InboxParts, type InboxRow, type ShelfKey } from './inbox';
  import Row from './rows/Row.svelte';
  import Shelf from './rows/Shelf.svelte';

  interface Props {
    parts: InboxParts;
    now: Date;
    chosen: string | null;
    selected: ReadonlySet<string>;
    renaming: string | null;
    marksOff: string | null;
    shelves: ReadonlySet<ShelfKey>;
    onToggleShelf: (key: ShelfKey) => void;
    onRowClick: (id: string, event: MouseEvent) => void;
    onMenu: (item: InboxRow, anchor: Anchor) => void;
    onAct: (item: InboxRow, action: RowAction) => void;
    onRename: (item: InboxRow, title: string | null) => void;
  }

  const { parts, now, chosen, selected, renaming, marksOff, shelves, onToggleShelf, onRowClick, onMenu, onAct, onRename }: Props = $props();
  const id = $props.id();
  /** The first page of Settled holds 10; each "Show more" adds up to 25 (02 section 5.7). */
  let settledShown = $state(10);
  /** Working and Snoozed show when they hold a thread; Settled always shows, as the foot of the list (08 inbox 21). */
  const keys = $derived((['working', 'snoozed', 'settled'] as const).filter((key) => key === 'settled' || parts[key].length > 0));
</script>

{#snippet row(item: InboxRow, kind: 'card' | 'slim', shelf: 'list' | 'snoozed' | 'settled')}
  <Row
    {item}
    {kind}
    {shelf}
    {now}
    open={item.row.id === chosen}
    selected={selected.has(item.row.id)}
    renaming={item.row.id === renaming}
    {marksOff}
    onclick={(event) => {
      onRowClick(item.row.id, event);
    }}
    onMenu={(anchor) => {
      onMenu(item, anchor);
    }}
    onAct={(action) => {
      onAct(item, action);
    }}
    onRename={(title) => {
      onRename(item, title);
    }}
  />
{/snippet}

<!-- The flat list (02 section 5.4): pinned and active cards with no heading, then the shelves at the foot. -->
<ul class="list" aria-label="Threads">
  {#each parts.pinned as item (item.row.id)}{@render row(item, 'card', 'list')}{/each}
  {#each parts.active as item (item.row.id)}{@render row(item, 'card', 'list')}{/each}
  {#each keys as key, at (key)}
    <Shelf
      title={SHELF_TITLES[key]}
      count={parts[key].length}
      open={shelves.has(key)}
      first={at === 0}
      controls="{id}-{key}"
      onToggle={() => {
        onToggleShelf(key);
      }}
    />
    {#if shelves.has(key)}
      <li class="shelf-rows" id="{id}-{key}">
        <ul aria-label={SHELF_TITLES[key]}>
          {#if key === 'working'}
            {#each parts.working as item (item.row.id)}{@render row(item, 'card', 'list')}{/each}
          {:else if key === 'snoozed'}
            {#each parts.snoozed as item (item.row.id)}{@render row(item, 'slim', 'snoozed')}{/each}
          {:else}
            {#each parts.settled.slice(0, settledShown) as item (item.row.id)}{@render row(item, 'slim', 'settled')}{/each}
            {#if parts.settled.length > settledShown}
              <li class="more-row">
                <button
                  type="button"
                  class="more"
                  onclick={() => {
                    settledShown += 25;
                  }}><CodeIcon name="chevron-down" />Show {Math.min(parts.settled.length - settledShown, 25)} more</button
                >
              </li>
            {/if}
          {/if}
        </ul>
      </li>
    {/if}
  {/each}
</ul>

<style>
  .list,
  .shelf-rows ul {
    display: flex;
    flex-direction: column;
    gap: var(--border-thin);
    margin: 0;
    padding: 0;
  }

  .list {
    flex: 1;
    padding: var(--space-2);
  }

  .shelf-rows,
  .more-row {
    list-style: none;
  }

  .more {
    display: flex;
    gap: var(--code-row-pad-inline);
    align-items: center;
    inline-size: 100%;
    block-size: var(--code-slim);
    padding: 0 var(--code-row-pad-inline);
    border: 0;
    border-radius: var(--radius-md);
    background: none;
    color: var(--code-quiet-text);
    font: inherit;
    font-size: var(--code-text-body);
    cursor: pointer;
  }

  .more:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  .more:focus-visible {
    outline: none;
    box-shadow: inset 0 0 0 var(--border-focus) var(--focus-ring);
  }
</style>
