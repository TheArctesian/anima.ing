<script lang="ts">
  import { popover } from '$lib/motion/roles/transitions';
  import { press } from '$lib/motion/roles/press';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import { menuPlace } from '../../kit/menu';
  import Tooltip from '../../kit/Tooltip.svelte';
  import type { Project } from '../inbox';
  import { picked, shownProjects } from './filter';

  interface Props {
    projects: readonly Project[];
    chosen: ReadonlySet<string | null>;
    onChoose: (next: ReadonlySet<string | null>) => void;
    /** Open from the start, for the specimen and the gallery. */
    open?: boolean;
  }

  let { projects, chosen, onChoose, open = $bindable(false) }: Props = $props();
  const id = $props.id();
  let trigger = $state<HTMLButtonElement>();
  let panel = $state<HTMLElement>();
  let query = $state('');
  /** The highlighted row: 0 is "All projects", then the shown projects. */
  let at = $state(0);
  const shown = $derived(shownProjects(projects, query));
  const label = $derived(chosen.size === 0 ? 'Filter threads by project' : `Filter threads by project, ${String(chosen.size)} chosen`);

  function choose(index: number, many: boolean): void {
    const project = index === 0 ? undefined : shown[index - 1];
    onChoose(picked(chosen, project === undefined ? undefined : project.folder, many));
    if (!many) close(true);
  }

  function close(back: boolean): void {
    open = false;
    query = '';
    at = 0;
    if (back) trigger?.focus();
  }

  function onkeydown(event: KeyboardEvent): void {
    const last = shown.length;
    if (event.key === 'ArrowDown') at = at >= last ? 0 : at + 1;
    else if (event.key === 'ArrowUp') at = at <= 0 ? last : at - 1;
    else if (event.key === 'Enter') choose(at, event.shiftKey);
    else if (event.key === 'Escape') close(true);
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

<!-- The project filter (02 section 5.3): a click picks one project; Shift-click or Shift+Enter adds or removes one. -->
<Tooltip text="Filter threads by project">
  <button
    type="button"
    class="trigger"
    class:on={chosen.size > 0}
    aria-label={label}
    aria-haspopup="listbox"
    aria-expanded={open}
    bind:this={trigger}
    onclick={() => {
      if (open) close(false);
      else open = true;
    }}
    {@attach press()}
  >
    <CodeIcon name="list-filter" />{#if chosen.size > 0}<span class="badge" aria-hidden="true">{chosen.size}</span>{/if}
  </button>
</Tooltip>
{#if open}
  <div class="panel" bind:this={panel} transition:popover={{ from: trigger }} {@attach place}>
    <label class="search">
      <CodeIcon name="search" />
      <input
        type="text"
        role="combobox"
        aria-label="Search projects"
        aria-expanded="true"
        aria-controls="{id}-list"
        aria-activedescendant="{id}-{at}"
        aria-autocomplete="list"
        placeholder="Search projects..."
        autocomplete="off"
        bind:value={query}
        oninput={() => (at = 0)}
        {onkeydown}
      />
    </label>
    <ul id="{id}-list" role="listbox" aria-label="Projects" aria-multiselectable="true">
      {#snippet option(index: number, name: string, selected: boolean, count: number | null)}
        <!-- Keys stay in the search field (aria-activedescendant); a pointer picks the row. -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <li
          id="{id}-{index}"
          role="option"
          aria-selected={selected}
          class:at={index === at}
          onpointerenter={() => (at = index)}
          onclick={(event) => {
            choose(index, event.shiftKey);
          }}
        >
          <span class="mark"><CodeIcon name="folder" />{#if selected}<span class="tick"><CodeIcon name="check" /></span>{/if}</span>
          <span class="name">{name}</span>
          {#if count !== null}<span class="count">{count}</span>{/if}
        </li>
      {/snippet}
      {@render option(0, 'All projects', chosen.size === 0, null)}
      {#each shown as project, index (project.folder)}
        {@render option(index + 1, project.name, chosen.has(project.folder), project.count)}
      {/each}
    </ul>
    {#if shown.length === 0}<p class="none">No matching projects.</p>{/if}
  </div>
{/if}

<style>
  .trigger {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    inline-size: var(--code-control-dense);
    block-size: var(--code-control-dense);
    border: 0;
    border-radius: var(--radius-md);
    background: none;
    color: var(--code-quiet-text);
    cursor: pointer;
  }

  .trigger:hover,
  .trigger[aria-expanded='true'] {
    background: var(--surface-hover);
    color: var(--text);
  }

  .on {
    color: var(--accent);
  }

  .trigger:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .badge {
    position: absolute;
    inset-block-end: 0;
    inset-inline-end: 0;
    font-size: var(--text-xs);
    font-weight: var(--weight-medium);
    line-height: 1;
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
    font-size: var(--code-text-body);
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
    font-size: var(--code-text-body);
    list-style: none;
    cursor: pointer;
  }

  li.at {
    background: var(--surface-hover);
  }

  .mark {
    position: relative;
    display: inline-flex;
    color: var(--code-quiet-text);
  }

  .tick {
    position: absolute;
    inset-block-end: calc(var(--space-1) * -1);
    inset-inline-end: calc(var(--space-1) * -1);
    display: inline-flex;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 var(--border-focus) var(--surface-overlay);
    color: var(--on-accent);
  }

  .tick :global(.code-icon) {
    inline-size: var(--space-3);
    block-size: var(--space-3);
  }

  .name {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .count,
  .none {
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    font-variant-numeric: tabular-nums;
  }

  .none {
    margin: 0;
    padding: 0 var(--space-3) var(--space-3);
  }
</style>
