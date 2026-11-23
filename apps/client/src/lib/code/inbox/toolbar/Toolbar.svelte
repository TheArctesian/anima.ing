<script lang="ts">
  import { press } from '$lib/motion/roles/press';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';
  import type { Project } from '../inbox';
  import ProjectFilter from './ProjectFilter.svelte';

  interface Props {
    search: string;
    /** The search field, for the key "/" of the page. */
    field?: HTMLInputElement | undefined;
    projects: readonly Project[];
    chosen: ReadonlySet<string | null>;
    onProjects: (next: ReadonlySet<string | null>) => void;
    /** The filter's switch for the sessions that Anima did not start; none when no such session is listed. */
    outside?: { readonly shown: boolean; readonly onShow: (shown: boolean) => void } | undefined;
    /** Why no thread can start here; `null` when one can. */
    newOff: string | null;
    onNew: () => void;
    /** Keys in the field: the arrows and Enter move through the results (02 section 5.10). */
    onFieldKey?: (event: KeyboardEvent) => void;
    /** The ID of the highlighted search result, while there is one. */
    activeId?: string | undefined;
  }

  let { search = $bindable(''), field = $bindable(), projects, chosen, onProjects, outside, newOff, onNew, onFieldKey, activeId }: Props = $props();
</script>

<!-- The toolbar of the inbox (02 section 5.3): the search, then the project filter and New thread. It does not scroll. -->
<div class="toolbar">
  <label class="search">
    <CodeIcon name="search" />
    <!-- A combobox over the search results (09 finding 10): the arrows move the highlight, the focus stays here. -->
    <input
      type="search"
      role="combobox"
      placeholder="Search"
      aria-label="Search threads"
      aria-autocomplete="list"
      aria-expanded={search.trim() !== ''}
      aria-controls={search.trim() !== '' ? 'inbox-search-results' : undefined}
      autocomplete="off"
      spellcheck="false"
      bind:value={search}
      bind:this={field}
      onkeydown={onFieldKey}
      aria-activedescendant={activeId}
    />
    {#if search !== ''}
      <button
        type="button"
        class="clear"
        aria-label="Clear thread search"
        onclick={() => {
          search = '';
          field?.focus();
        }}><CodeIcon name="x" /></button
      >
    {/if}
  </label>
  <!-- T3: 4 px after the search field, no gap between the buttons (08 inbox 25). -->
  <span class="group">
  {#if projects.length > 0 || outside !== undefined}<ProjectFilter {projects} {chosen} onChoose={onProjects} {outside} />{/if}
  <Tooltip text={newOff === null ? 'New thread (⌘N)' : `New thread: ${newOff}`}>
    <button
      type="button"
      class="square"
      aria-label="New thread"
      aria-keyshortcuts="Meta+N"
      aria-disabled={newOff === null ? undefined : 'true'}
      onclick={() => {
        if (newOff === null) onNew();
      }}
      {@attach press()}><CodeIcon name="square-pen" /></button
    >
  </Tooltip>
  </span>
</div>

<style>
  .toolbar {
    display: flex;
    flex: none;
    gap: var(--space-1);
    align-items: center;
    padding: var(--space-2);
  }

  .group {
    display: flex;
    flex: none;
    align-items: center;
  }

  .search {
    display: flex;
    flex: 1;
    gap: var(--space-2);
    align-items: center;
    min-inline-size: 0;
    block-size: var(--code-control);
    padding: calc(var(--space-1) * 1.5) var(--space-2);
    border-radius: var(--radius-md);
    color: var(--code-quiet-text);
    font-size: var(--code-text-body);
    font-weight: var(--weight-medium);
  }

  .search:hover,
  .search:focus-within {
    background: var(--surface-hover);
    color: var(--text);
  }

  /* The 2 px ring of the other controls (09 finding 35). */
  .search:focus-within {
    box-shadow: inset 0 0 0 var(--border-focus) var(--focus-ring);
  }

  input {
    flex: 1;
    min-inline-size: 0;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    font: inherit;
    appearance: none;
  }

  input::-webkit-search-cancel-button {
    appearance: none;
  }

  input:focus-visible {
    outline: none;
  }

  input::placeholder {
    color: var(--code-quiet-text);
  }

  button {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    border: 0;
    border-radius: var(--radius-md);
    background: none;
    color: var(--code-quiet-text);
    cursor: pointer;
  }

  /* 24 px, the smallest target (09 finding 21). */
  .clear {
    inline-size: var(--space-6);
    block-size: var(--space-6);
  }

  .clear :global(.code-icon) {
    inline-size: var(--text-xs);
    block-size: var(--text-xs);
  }

  .square {
    inline-size: var(--code-control-dense);
    block-size: var(--code-control-dense);
  }

  button:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  button[aria-disabled='true'] {
    opacity: var(--opacity-disabled);
    cursor: default;
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }
</style>
