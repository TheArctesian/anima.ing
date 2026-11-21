<script lang="ts">
  import CodeIcon from '../icons/CodeIcon.svelte';
  import { shortcutOf } from './table';
  import type { PaletteGroup, PaletteRow } from './results';

  interface Props {
    /** The id of the listbox, which the input names as the list it controls. */
    id: string;
    groups: readonly PaletteGroup[];
    /** The index of the shaded row among all rows, or -1. */
    active: number;
    mac: boolean;
    /** The id of the row at `index`, for `aria-activedescendant`. */
    rowId: (index: number) => string;
    onpoint: (index: number) => void;
    onchoose: (row: PaletteRow) => void;
  }

  const { id, groups, active, mac, rowId, onpoint, onchoose }: Props = $props();

  /** Where each group starts among all rows. */
  const starts = $derived(groups.reduce<number[]>((at, group, index) => [...at, index === 0 ? 0 : (at[index - 1] ?? 0) + (groups[index - 1]?.rows.length ?? 0)], []));
</script>

<!-- The results (02 section 10.2): groups with a label, one shaded row at a time; the pointer moves the same shading. -->
<!-- A press on a row keeps the focus in the input, so the keys still reach the palette after a click. -->
<div
  {id}
  class="list"
  role="listbox"
  tabindex="-1"
  aria-label="Results"
  onmousedown={(event) => {
    event.preventDefault();
  }}
>
  {#each groups as group, groupIndex (group.id)}
    <div class="group" role="group" aria-labelledby="{id}-{group.id}">
      <div class="label" id="{id}-{group.id}">{group.label}</div>
      {#each group.rows as row, rowIndex (row.id)}
        {@const index = (starts[groupIndex] ?? 0) + rowIndex}
        {@const shortcut = row.command === undefined ? undefined : shortcutOf(row.command, mac)}
        <!-- The focus stays in the input, which takes the keys for the rows (a combobox, WAI-ARIA APG). -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          id={rowId(index)}
          class="row"
          class:active={index === active}
          role="option"
          tabindex="-1"
          aria-selected={index === active}
          aria-disabled={row.disabled || undefined}
          onpointermove={() => {
            if (index !== active) onpoint(index);
          }}
          onclick={() => {
            onchoose(row);
          }}
        >
          <span class="icon"><CodeIcon name={row.icon} /></span>
          <span class="words">
            <span class="title">{row.title}</span>
            {#if row.detail !== undefined}<span class="detail">{row.detail}</span>{/if}
          </span>
          {#if row.context !== undefined}<span class="context">{row.context}</span>{/if}
          {#if row.age !== undefined}<span class="age">{row.age}</span>{/if}
          {#if shortcut !== undefined}<span class="shortcut" aria-hidden="true">{shortcut}</span>{/if}
        </div>
      {/each}
    </div>
  {/each}
</div>

<style>
  .list {
    padding: var(--space-2);
  }

  .group + .group {
    margin-block-start: var(--code-palette-group-gap);
  }

  .label {
    padding: var(--code-palette-row-pad);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    font-weight: var(--weight-medium);
  }

  .row {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
    min-block-size: var(--code-palette-row);
    padding: var(--code-palette-row-pad);
    border-radius: var(--radius-sm);
    color: var(--text);
    font-size: var(--code-text-body);
    cursor: default;
  }

  .active {
    background: var(--surface-selected);
  }

  .icon {
    display: inline-flex;
    flex: none;
    align-items: center;
    block-size: var(--space-5);
    color: var(--code-quiet-text);
  }

  .words {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-inline-size: 0;
  }

  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* T3 Code fades a disabled row to 0.64; here its title takes the quiet text and the reason says why, at 4.5:1 (D9). */
  .row[aria-disabled='true'] .title {
    color: var(--code-quiet-text);
  }

  .detail,
  .context,
  .age,
  .shortcut {
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }

  .context {
    flex: none;
    max-inline-size: var(--code-palette-context);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .age {
    flex: none;
    min-inline-size: var(--code-palette-time);
    font-variant-numeric: tabular-nums;
    text-align: end;
  }

  .shortcut {
    flex: none;
    font-weight: var(--weight-medium);
    letter-spacing: var(--tracking-label);
  }

  .context,
  .age,
  .shortcut {
    line-height: var(--space-5);
  }
</style>
