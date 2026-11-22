<script lang="ts">
  import CodeIcon from '../icons/CodeIcon.svelte';
  import type { PaletteGroup, PaletteRow } from './results';

  interface Props {
    /** The id of the listbox, which the input names as the list it controls. */
    id: string;
    groups: readonly PaletteGroup[];
    /** The index of the shaded row among all rows, or -1. */
    active: number;
    /** The key a row shows for its command, if any. */
    shortcut: (command: string) => string | undefined;
    /** The id of the row at `index`, for `aria-activedescendant`. */
    rowId: (index: number) => string;
    onpoint: (index: number) => void;
    onchoose: (row: PaletteRow) => void;
  }

  const { id, groups, active, shortcut, rowId, onpoint, onchoose }: Props = $props();

  /** Where each group starts among all rows. */
  const starts = $derived(groups.reduce<number[]>((at, group, index) => [...at, index === 0 ? 0 : (at[index - 1] ?? 0) + (groups[index - 1]?.rows.length ?? 0)], []));
</script>

<!-- The results (02 section 10.2): groups with a label, one shaded row at a time; the pointer moves the same shading. -->
<!-- A disabled row takes no shading and shows no key (T3 Code's `DisabledCommandPaletteResultRow`). -->
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
        {@const key = row.command === undefined || row.disabled ? undefined : shortcut(row.command)}
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
            if (index !== active && !row.disabled) onpoint(index);
          }}
          onclick={() => {
            onchoose(row);
          }}
        >
          <span class="icon"><CodeIcon name={row.icon} /></span>
          <span class="words">
            <span class="title">{row.title}</span>
            {#if row.detail !== undefined}<span class="detail" class:subtitle={row.id.startsWith('chat:')}>{row.detail}</span>{/if}
          </span>
          {#if row.age !== undefined}<span class="age">{row.age}</span>{/if}
          {#if key !== undefined}<span class="shortcut" aria-hidden="true">{key}</span>{/if}
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
    align-items: center;
    gap: var(--space-2);
    min-block-size: var(--code-palette-row-narrow);
    padding: var(--code-palette-row-pad);
    border-radius: var(--radius-sm);
    color: var(--text);
    font-size: var(--code-text-body);
    cursor: default;
  }

  .active {
    background: var(--surface-selected);
  }

  /* Only the icon sits at the top, 2 px down, so it stays on the title of a row of two lines. */
  .icon {
    display: inline-flex;
    flex: none;
    align-items: center;
    align-self: flex-start;
    block-size: var(--space-5);
    padding-block-start: var(--code-palette-icon-inset);
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
  .age,
  .shortcut {
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }

  /* A chat's subtitle keeps one line (T3 Code's `ThreadCommandSubtitle`); a reason wraps. */
  .subtitle {
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

  @media (width >= 40rem) {
    .row {
      min-block-size: var(--code-palette-row);
    }
  }
</style>
