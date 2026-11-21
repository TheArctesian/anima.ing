<script lang="ts">
  import '../picker.css';
  import CodeIcon from '../../../icons/CodeIcon.svelte';
  import ScrollArea from '../../../kit/ScrollArea.svelte';
  import Tooltip from '../../../kit/Tooltip.svelte';
  import type { Model, Row } from '../catalogue';
  import ModelRow from './ModelRow.svelte';

  interface Props {
    rows: readonly Row[];
    /** The key of the lit row: the one Enter picks. */
    lit: string | null;
    /** The key of the model in use; its row carries the check. */
    chosen: string;
    query: string;
    /** The words of each jump key, by the key of its row. */
    jumps: ReadonlyMap<string, string>;
    /** In the favourites tab each row is a favourite, so its mark shows only when the row is pointed at. */
    quietMarks: boolean;
    /** What the list says when it has no row. */
    empty: string;
    onSearchKey: (event: KeyboardEvent) => void;
    onLight: (key: string) => void;
    onActivate: (row: Row) => void;
    /** Left out, a favourite only carries its mark and nothing can be changed. */
    onFavourite?: ((model: Model, favourite: boolean) => void) | undefined;
  }

  let { rows, lit, chosen, query = $bindable(), jumps, quietMarks, empty, onSearchKey, onLight, onActivate, onFavourite }: Props = $props();
  const id = $props.id();
  let input: HTMLInputElement | undefined;
  const litAt = $derived(rows.findIndex((row) => row.key === lit));

  /** Puts keyboard focus in the search field. */
  export function focus(): void {
    input?.focus({ preventScroll: true });
  }

  /** The caret of the search field; null while a range is selected. */
  export function caret(): number | null {
    return input === undefined || input.selectionStart !== input.selectionEnd ? null : input.selectionStart;
  }

  /* The lit row stays in view while the keys move it (03 A.2.5): no animation. */
  $effect(() => {
    if (litAt >= 0) document.getElementById(`${id}-row-${String(litAt)}`)?.scrollIntoView?.({ block: 'nearest' });
  });
</script>

{#snippet modelRow(row: Extract<Row, { kind: 'model' }>, index: number)}
  <!-- No key event on a row: the search field holds focus and sends Enter for the lit row. -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="row"
    class:lit={row.key === lit}
    id="{id}-row-{index}"
    role="option"
    tabindex="-1"
    aria-selected={row.key === chosen}
    aria-disabled={row.model.can.can ? undefined : 'true'}
    onpointermove={() => { onLight(row.key); }}
    onclick={() => { onActivate(row); }}
  >
    <ModelRow {row} chosen={row.key === chosen} jump={jumps.get(row.key)} quietMark={quietMarks} {onFavourite} />
  </div>
{/snippet}

<!-- The search field and the list (03 A.2.4): a combobox. Focus stays in the field; Up and Down move the lit row. -->
<div class="column">
  <label class="search">
    <span class="field">
      <CodeIcon name="search" />
      <input
        type="text"
        role="combobox"
        aria-label="Search models"
        aria-expanded="true"
        aria-controls="{id}-list"
        aria-autocomplete="list"
        aria-activedescendant={litAt >= 0 ? `${id}-row-${String(litAt)}` : undefined}
        placeholder="Search models..."
        autocomplete="off"
        spellcheck="false"
        bind:value={query}
        bind:this={input}
        onkeydown={onSearchKey}
      />
    </span>
  </label>
  <div class="list" class:none={rows.length === 0}>
    <ScrollArea label="Models">
      <div class="rows" id="{id}-list" role="listbox" aria-label="Models">
        {#each rows as row, index (row.key)}
          {#if row.kind === 'model'}
            {#if row.model.can.can}
              {@render modelRow(row, index)}
            {:else}
              <Tooltip text={row.model.can.reason}>{@render modelRow(row, index)}</Tooltip>
            {/if}
          {:else}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div
              class="row more"
              class:lit={row.key === lit}
              id="{id}-row-{index}"
              role="option"
              tabindex="-1"
              aria-selected="false"
              onpointermove={() => { onLight(row.key); }}
              onclick={() => { onActivate(row); }}
            >
              {row.expanded ? 'Show less' : 'Show more'}
            </div>
          {/if}
        {/each}
      </div>
    </ScrollArea>
  </div>
  {#if rows.length === 0}<p class="empty" role="status">{empty}</p>{/if}
</div>

<style>
  .column {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-inline-size: 0;
    min-block-size: 0;
  }

  /* The search line: its rule runs on into the head of the effort column (03 A.3.3). */
  .search {
    display: block;
    flex: none;
    padding: var(--code-composer-picker-head-pad) var(--space-3) 0;
    cursor: text;
  }

  .field {
    display: flex;
    gap: var(--space-1);
    align-items: center;
    padding-block-end: var(--code-composer-picker-rule-gap);
    border-block-end: var(--border-thin) solid var(--code-composer-picker-line);
    color: var(--code-composer-picker-quiet);
  }

  .field:focus-within {
    border-block-end-color: var(--focus-ring);
    box-shadow: 0 var(--border-thin) 0 0 var(--focus-ring);
  }

  input {
    flex: 1;
    min-inline-size: 0;
    block-size: var(--code-composer-picker-search);
    padding: 0;
    border: 0;
    outline: none;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: var(--code-text-body);
    font-family: var(--font-ui);
  }

  input::placeholder {
    color: var(--code-composer-picker-quiet);
  }

  .list {
    flex: 1;
    min-block-size: 0;
  }

  .list.none {
    display: none;
  }

  .list > :global(.area) {
    block-size: 100%;
    scrollbar-gutter: stable;
  }

  /* 10 px over and under the rows, 12 px before them and 5 px after (the source's list and its 4 px frame). */
  .rows {
    display: flex;
    flex-direction: column;
    gap: var(--code-composer-picker-row-gap);
    padding: var(--code-composer-picker-head-pad) calc(var(--space-1) + var(--border-thin)) var(--code-composer-picker-head-pad) var(--space-3);
  }

  /* A row that cannot be picked sits in the anchor of its tip; it stays as wide as the others. */
  .rows > :global(.anchor) {
    display: flex;
  }

  .row {
    display: flex;
    gap: var(--space-2);
    align-items: center;
    min-inline-size: 0;
    min-block-size: var(--code-composer-picker-row);
    padding: var(--space-1) var(--space-2);
    border-radius: var(--radius-sm);
    color: var(--text);
    font-size: var(--code-text-body);
    cursor: pointer;
  }

  .row.lit {
    background: var(--surface-hover);
  }

  .row[aria-disabled='true'] {
    flex: 1;
    opacity: var(--opacity-disabled);
    cursor: not-allowed;
  }

  .more {
    flex: none;
    color: var(--code-composer-picker-quiet);
  }

  .more.lit {
    color: var(--text);
  }

  .empty {
    flex: 1;
    margin: 0;
    padding: var(--space-2);
    color: var(--code-composer-picker-quiet);
    font-size: var(--code-text-body);
    text-align: center;
  }
</style>
