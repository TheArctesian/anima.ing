<script lang="ts">
  import Switch from '$lib/forms/switch/Switch.svelte';
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
    /** The switch for the sessions that Anima did not start; none when no such session is listed. */
    outside?: { readonly shown: boolean; readonly onShow: (shown: boolean) => void } | undefined;
    /** Open from the start, for the specimen and the gallery. */
    open?: boolean;
  }

  let { projects, chosen, onChoose, outside, open = $bindable(false) }: Props = $props();
  const id = $props.id();
  let trigger = $state<HTMLButtonElement>();
  let panel = $state<HTMLElement>();
  let query = $state('');
  /** The highlighted row: 0 is "All projects", then the shown projects. */
  let at = $state(0);
  const shown = $derived(shownProjects(projects, query));
  const names = $derived(projects.filter((project) => chosen.has(project.folder)).map((project) => project.name));
  const label = $derived(names.length === 0 ? 'Filter threads by project' : `Filter threads by project: ${names.join(', ')}`);

  /** Focus goes back to the trigger only after a choice by the keys (T3's finalFocus). */
  function choose(index: number, many: boolean, keys = false): void {
    const project = index === 0 ? undefined : shown[index - 1];
    onChoose(picked(chosen, project === undefined ? undefined : project.folder, many));
    if (!many) close(keys);
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
    else if (event.key === 'Enter') choose(at, event.shiftKey, true);
    // Tab goes on to the switch under the list; otherwise Tab and Escape close the menu and give focus back to the
    // trigger, so it never stays open behind the focus (09 finding 18).
    else if (event.key === 'Tab' && !event.shiftKey && outside !== undefined) return;
    else if (event.key === 'Escape' || event.key === 'Tab') close(true);
    else return;
    event.preventDefault();
  }

  function onfocusout(event: FocusEvent): void {
    const next = event.relatedTarget;
    if (!open || !(next instanceof Node) || panel?.contains(next) === true || trigger?.contains(next) === true) return;
    close(false);
  }

  // The highlighted row stays in sight in the scrolling list (09 finding 18).
  $effect(() => {
    if (open) document.getElementById(`${id}-${String(at)}`)?.scrollIntoView?.({ block: 'nearest' });
  });

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
    <!-- T3: the filter icon; the project's icon when one is chosen; two overlapping when more are (08 inbox 23). -->
    {#if names.length === 0}<CodeIcon name="list-filter" />{:else if names.length === 1}<CodeIcon name="folder" />{:else}<span class="pair"><CodeIcon name="folder" /><CodeIcon name="folder" /></span>{/if}
  </button>
</Tooltip>
{#if open}
  <div class="panel" bind:this={panel} {onfocusout} transition:popover={{ from: trigger }} {@attach place}>
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
      {#snippet option(index: number, name: string, selected: boolean)}
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
        </li>
      {/snippet}
      {@render option(0, 'All projects', chosen.size === 0)}
      {#each shown as project, index (project.folder)}
        {@render option(index + 1, project.name, chosen.has(project.folder))}
      {/each}
    </ul>
    {#if shown.length === 0}<p class="none">No matching projects.</p>{/if}
    {#if outside !== undefined}
      <!-- Sessions that Anima did not start are hidden until the person asks for them (operator, 2026-11-23). -->
      <div
        class="outside"
        role="presentation"
        onkeydown={(event) => {
          if (event.key !== 'Escape' && (event.key !== 'Tab' || event.shiftKey)) return;
          event.preventDefault();
          close(true);
        }}
      >
        <span id="{id}-outside">Browse imported history</span>
        <Switch checked={outside.shown} labelledby="{id}-outside" onchange={(next) => { outside?.onShow(next); }} />
      </div>
    {/if}
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

  .trigger:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .pair {
    display: inline-flex;
  }

  .pair :global(.code-icon) {
    inline-size: var(--code-icon-small);
    block-size: var(--code-icon-small);
  }

  .pair :global(.code-icon + .code-icon) {
    margin-inline-start: calc(var(--space-1) * -1.5);
  }

  .outside {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    border-block-start: var(--border-thin) solid var(--border-strong);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
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

  .none {
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    margin: 0;
    padding: 0 var(--space-3) var(--space-3);
  }
</style>
