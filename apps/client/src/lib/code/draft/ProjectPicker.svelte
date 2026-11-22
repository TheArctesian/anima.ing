<script lang="ts">
  import { popover } from '$lib/motion/roles/transitions';
  import CodeIcon from '../icons/CodeIcon.svelte';
  import type { Project } from '../inbox/inbox';
  import { shownProjects } from '../inbox/toolbar/filter';
  import { menuPlace } from '../kit/menu';
  import { addedFolder } from './project';

  interface Props {
    /** The projects of this computer: the folders of its threads. */
    projects: readonly Project[];
    /** The folder of the draft; null when none is chosen. */
    folder: string | null;
    /** The trigger's words: the project's name, "Choose a project" or "Add a project". */
    words: string;
    onChoose: (folder: string) => void;
    /** Open from the start, for the gallery. */
    open?: boolean;
  }

  let { projects, folder, words, onChoose, open = $bindable(false) }: Props = $props();
  const id = $props.id();
  let trigger = $state<HTMLButtonElement>();
  let panel = $state<HTMLElement>();
  let query = $state('');
  /** "Add project" was chosen. */
  let chose = $state(false);
  /** The field takes a folder's path: "Add project" was chosen, or there is no project to list. */
  const adding = $derived(chose || projects.length === 0);
  let problem = $state<string | null>(null);
  let at = $state(0);
  const shown = $derived(shownProjects(projects, query));
  /** The rows: the shown projects, then "Add project" when nothing is searched (T3 hides it from a search). */
  const last = $derived(query.trim() === '' ? shown.length : shown.length - 1);

  function close(back: boolean): void {
    open = false;
    query = '';
    at = 0;
    problem = null;
    chose = false;
    if (back) trigger?.focus();
  }

  function choose(index: number, keys: boolean): void {
    const project = shown[index];
    if (project === undefined) {
      chose = true;
      query = '';
      return;
    }
    if (project.folder !== null) onChoose(project.folder);
    close(keys);
  }

  function add(): void {
    const added = addedFolder(query);
    if (!added.ok) {
      problem = added.words;
      return;
    }
    onChoose(added.folder);
    close(true);
  }

  function onkeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape' || event.key === 'Tab') close(true);
    else if (adding && event.key === 'Enter') add();
    else if (adding) return;
    else if (event.key === 'ArrowDown') at = at >= last ? 0 : at + 1;
    else if (event.key === 'ArrowUp') at = at <= 0 ? last : at - 1;
    else if (event.key === 'Enter') choose(at, true);
    else return;
    event.preventDefault();
  }

  function onOutside(event: PointerEvent): void {
    if (!open || !(event.target instanceof Node) || panel?.contains(event.target) === true || trigger?.contains(event.target) === true) return;
    close(false);
  }

  function place(node: HTMLElement): void {
    if (trigger === undefined) return;
    const spot = menuPlace(
      trigger.getBoundingClientRect(),
      { width: node.offsetWidth, height: node.offsetHeight },
      { width: document.documentElement.clientWidth, height: document.documentElement.clientHeight },
      { side: 'below', align: 'start', gap: Number.parseFloat(getComputedStyle(node).marginBlockStart) || 0 },
    );
    node.style.insetInlineStart = `${String(spot.left)}px`;
    node.style.insetBlockStart = `${String(spot.top)}px`;
    node.querySelector('input')?.focus();
  }
</script>

<svelte:document onpointerdown={onOutside} />

<!-- T3 Code's project picker of a draft (`DraftHeroHeadline.tsx`): the project's name as a dotted link, a list with search, and "Add project". -->
<button type="button" class="trigger" aria-haspopup="listbox" aria-expanded={open} title={folder ?? undefined} bind:this={trigger} onclick={() => (open ? close(false) : (open = true))}>{words}</button>
{#if open}
  <div class="panel" bind:this={panel} transition:popover={{ from: trigger }} {@attach place}>
    <label class="search">
      <CodeIcon name={adding ? 'folder-plus' : 'search'} />
      {#if adding}
        <input type="text" aria-label="Folder of the project" placeholder="/Users/you/project" autocomplete="off" spellcheck="false" bind:value={query} oninput={() => (problem = null)} {onkeydown} />
      {:else}
        <input type="text" role="combobox" aria-label="Search projects" aria-expanded="true" aria-controls="{id}-list" aria-activedescendant="{id}-{at}" aria-autocomplete="list" placeholder="Search projects..." autocomplete="off" bind:value={query} oninput={() => (at = 0)} {onkeydown} />
      {/if}
    </label>
    {#if adding}
      <p class="hint" class:problem={problem !== null} role={problem === null ? undefined : 'alert'}>{problem ?? 'An absolute folder of this computer. Press Enter to add it.'}</p>
    {:else}
      <ul id="{id}-list" role="listbox" aria-label="Projects">
        {#snippet row(index: number, name: string, icon: 'folder' | 'folder-plus', selected: boolean)}
          <!-- Keys stay in the search field (aria-activedescendant); a pointer picks the row. -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <li id="{id}-{index}" role="option" aria-selected={selected} class:at={index === at} onpointerenter={() => (at = index)} onclick={() => choose(index, false)}>
            <CodeIcon name={icon} /><span class="name">{name}</span>
          </li>
        {/snippet}
        {#each shown as project, index (project.folder)}{@render row(index, project.name, 'folder', project.folder === folder)}{/each}
        {#if query.trim() === ''}{@render row(shown.length, 'Add project', 'folder-plus', false)}{/if}
      </ul>
      {#if shown.length === 0 && query.trim() !== ''}<p class="hint">No matching projects.</p>{/if}
    {/if}
  </div>
{/if}

<style>
  /* T3's InlineButton tone "picker": the words with a dotted line under them. */
  .trigger {
    display: inline;
    max-inline-size: 100%;
    padding: 0;
    overflow: hidden;
    border: 0;
    border-block-end: var(--border-thin) dotted var(--code-quiet-text);
    background: none;
    color: inherit;
    font: inherit;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: pointer;
  }

  .trigger:hover,
  .trigger[aria-expanded='true'] {
    border-block-end-color: var(--text);
  }

  .trigger:focus-visible {
    border-radius: var(--radius-sm);
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: var(--border-focus);
  }

  .panel {
    position: fixed;
    z-index: var(--layer-popover);
    inline-size: min(calc(var(--space-16) * 4.5), calc(100vw - var(--space-8)));
    margin-block-start: var(--code-offset);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    background: var(--surface-overlay);
    box-shadow: var(--elevation-2);
    font-size: var(--code-text-body);
    font-weight: var(--weight-regular);
    letter-spacing: normal;
    text-align: start;
  }

  .search {
    display: flex;
    gap: var(--space-2);
    align-items: center;
    padding: var(--space-2) var(--space-3);
    border-block-end: var(--border-thin) solid var(--border-strong);
    color: var(--code-quiet-text);
  }

  .search:focus-within {
    border-block-end-color: var(--focus-ring);
  }

  input {
    flex: 1;
    min-inline-size: 0;
    border: 0;
    background: none;
    color: var(--text);
    font: inherit;
  }

  input:focus-visible {
    outline: none;
  }

  input::placeholder {
    color: var(--code-quiet-text);
  }

  ul {
    max-block-size: calc(var(--space-16) * 5);
    margin: 0;
    padding: var(--code-menu-pad);
    overflow-y: auto;
  }

  li {
    display: flex;
    gap: var(--space-2);
    align-items: center;
    min-block-size: var(--code-menu-item);
    padding: var(--space-1) var(--space-2);
    border-radius: var(--radius-sm);
    color: var(--text);
    list-style: none;
    cursor: pointer;
  }

  li.at {
    background: var(--surface-hover);
  }

  li :global(.code-icon) {
    color: var(--code-quiet-text);
  }

  .name {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .hint {
    margin: 0;
    padding: var(--space-2) var(--space-3);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }

  .hint.problem {
    color: var(--status-down);
  }
</style>
