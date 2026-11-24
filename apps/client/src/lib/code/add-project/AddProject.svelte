<script lang="ts">
  import '../palette/palette.css';
  import CodeIcon from '../icons/CodeIcon.svelte';
  import { openDialog } from '$lib/forms/modal/stack';
  import { fade, modal } from '$lib/motion/roles/transitions';
  import { coverWhile } from '$lib/overlay/overlay.svelte';
  import PaletteFoot from '../palette/PaletteFoot.svelte';
  import PaletteResults from '../palette/PaletteResults.svelte';
  import { firstEnabled, stepped, type PaletteRow } from '../palette/results';
  import { AddProjectWalk, type FolderList } from './walk.svelte';

  interface Props {
    open?: boolean;
    /** The folders in a folder, from the runner's `folders` call. */
    list: (folder: string) => Promise<FolderList>;
    /** Where "Local folder" opens. */
    start: string;
    /** Takes the chosen folder; the palette closes. */
    onAdd: (folder: string) => void;
    onNew: () => void;
  }

  let { open = $bindable(false), list, start, onAdd, onNew }: Props = $props();
  const id = $props.id();

  const walk = new AddProjectWalk(
    (folder) => list(folder),
    () => start,
    (folder) => {
      open = false;
      onAdd(folder);
    },
    () => { open = false; onNew(); },
  );
  let input: HTMLInputElement | undefined = $state();
  let shadedId = $state<string | undefined>(undefined);

  const groups = $derived(walk.groups);
  const rows = $derived(groups.flatMap((group) => group.rows));
  const active = $derived.by(() => {
    const at = rows.findIndex((row) => row.id === shadedId && !row.disabled);
    return at >= 0 ? at : firstEnabled(rows);
  });
  const note = $derived(walk.loading ? 'Loading folders…' : walk.more ? 'Some folders are not shown: type a path.' : undefined);
  const rowId = (index: number): string => `${id}-row-${String(index)}`;

  coverWhile(() => open);

  /** Each opening starts at the sources with the focus in the search; closing puts the focus back where it was. */
  $effect.pre(() => {
    if (!open) return;
    const opened = openDialog();
    const before = document.activeElement;
    walk.reset();
    shadedId = undefined;
    return () => {
      opened.close();
      if (before instanceof HTMLElement && before.isConnected) before.focus();
    };
  });

  $effect(() => {
    if (open) input?.focus();
  });

  $effect(() => {
    if (active >= 0) document.getElementById(rowId(active))?.scrollIntoView({ block: 'nearest' });
  });

  function choose(row: PaletteRow | undefined): void {
    if (!open || row === undefined || row.disabled) return;
    shadedId = undefined;
    row.run();
  }

  function back(): void {
    shadedId = undefined;
    walk.back();
    input?.focus();
  }

  function onkey(event: KeyboardEvent): void {
    if (event.isComposing || event.keyCode === 229) return;
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') shadedId = rows[stepped(active, event.key === 'ArrowDown' ? 1 : -1, rows)]?.id;
    else if (event.key === 'Enter') choose(rows[active]);
    else if (event.key === 'Escape') open = false;
    else if (event.key === 'Backspace' && walk.query === '' && walk.folder !== null) back();
    else if (event.key !== 'Tab') return;
    event.preventDefault();
    event.stopPropagation();
  }

  function toBody(root: HTMLElement): () => void {
    document.body.append(root);
    return () => {
      root.remove();
    };
  }
</script>

<!-- T3 Code's "Add project" palette (`CommandPalette.tsx`): a back arrow and a search, the "Sources" group, then the folders of "Local folder". -->
{#if open}
  <div class="portal code-palette" {@attach toBody}>
    <div class="scrim" transition:fade|global></div>
    <div
      class="frame"
      role="presentation"
      onpointerdown={(event) => {
        if (event.target === event.currentTarget) open = false;
      }}
    >
      <div class="surface" role="dialog" aria-modal="true" aria-label="Add project" transition:modal|global>
        <div class="field">
          {#if walk.folder === null}
            <span class="search"><CodeIcon name="search" /></span>
          {:else}
            <button type="button" class="search back" aria-label="Back" onclick={back}><CodeIcon name="arrow-left" /></button>
          {/if}
          <input
            bind:this={input}
            bind:value={walk.query}
            oninput={() => {
              shadedId = undefined;
            }}
            onkeydown={onkey}
            role="combobox"
            aria-expanded={rows.length > 0}
            aria-controls={rows.length > 0 ? `${id}-list` : undefined}
            aria-activedescendant={rows[active] === undefined ? undefined : rowId(active)}
            aria-autocomplete="list"
            aria-label={walk.folder === null ? 'Search sources' : 'Search folders, or type a path'}
            placeholder={walk.folder === null ? 'Search...' : 'Search folders, or type a path...'}
            autocomplete="off"
            spellcheck="false"
          />
        </div>
        <div class="results">
          {#if walk.problem !== null}
            <p class="empty" role="alert">{walk.problem}</p>
          {:else if rows.length > 0}
            <PaletteResults id="{id}-list" {groups} {active} shortcut={() => undefined} {rowId} onpoint={(index) => (shadedId = rows[index]?.id)} onchoose={choose} />
          {:else if !walk.loading}
            <p class="empty">{walk.folder === null ? 'No matching sources.' : 'No matching folders.'}</p>
          {/if}
        </div>
        <p class="visually-hidden" role="status">{note ?? (rows.length === 1 ? '1 result' : `${String(rows.length)} results`)}</p>
        <PaletteFoot {note} back={walk.folder !== null} />
      </div>
    </div>
  </div>
{/if}

<style>
  /* The back arrow sits where the search icon is, and takes a press. */
  .search.back {
    padding: 0;
    border: 0;
    background: none;
    pointer-events: auto;
    cursor: pointer;
  }

  .search.back:hover {
    color: var(--text);
  }

  .search.back:focus-visible {
    border-radius: var(--radius-sm);
    outline: var(--border-focus) solid var(--focus-ring);
  }
</style>
