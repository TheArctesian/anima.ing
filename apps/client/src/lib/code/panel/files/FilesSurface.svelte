<script lang="ts">
  import type { Snippet } from 'svelte';
  import IconButton from '../../kit/IconButton.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';

  interface Props {
    /** The name of the project, for the name of the search field. */
    project: string;
    /** What is typed in the search field. */
    query: string;
    onquery: (query: string) => void;
    onrefresh: () => void;
    /** The folders or the search results load. */
    loading?: boolean;
    /** The tree has rows now; then "Loading files…" is said and not shown. */
    listed?: boolean;
    /** A read or a search failed; the words of the failure. A press reads again. */
    error?: string | undefined;
    onretry?: () => void;
    /** The search was cut short. */
    more?: boolean;
    /** The search has no match; the tree is not drawn. */
    noMatch?: boolean;
    /** Whether all folders are open; `none` hides the button (no folders, or a folder outside the workspace). */
    folds?: 'open' | 'closed' | 'none';
    onfold?: () => void;
    /** File access: still checked, there, or not there (with the words of the reason, if any). */
    access?: 'checking' | 'ready' | { readonly denied: string | undefined };
    /** The panel is a sheet over the chat: the header is 40px with a line under it, and not 36px with none. */
    sheet?: boolean;
    /** The tree of the files. It is not built here: it waits for the decision on a tree library. */
    tree: Snippet;
  }

  const {
    project, query, onquery, onrefresh, loading = false, listed = false, error, onretry, more = false, noMatch = false, folds = 'none', onfold,
    access = 'ready', sheet = false, tree,
  }: Props = $props();

  let field = $state<HTMLInputElement>();
  const foldWords = $derived(folds === 'open' ? 'Collapse all folders' : 'Expand all folders');

  /** Puts the focus in the search field and selects its text: the "find" action of the panel calls this. */
  export function focusSearch(): void {
    field?.focus();
    field?.select();
  }

  /** mod+F, with no Alt and no Shift, anywhere in the surface finds in the files. */
  function onkeydown(event: KeyboardEvent): void {
    if (!(event.metaKey || event.ctrlKey) || event.altKey || event.shiftKey || event.key.toLowerCase() !== 'f') return;
    event.preventDefault();
    focusSearch();
  }

  /** Escape in the field clears the search and leaves the field. */
  function onFieldKey(event: KeyboardEvent): void {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    event.stopPropagation();
    onquery('');
    field?.blur();
  }
</script>

<!--
  The Files surface of the right panel (report 05, Section 2): the header with refresh, search and fold-all, then the
  states and the tree. A page part: it takes what it shows and says what the person asks.
-->
<!-- svelte-ignore a11y_no_static_element_interactions (the surface takes mod+F for the search field in it) -->
<div class="files" data-file-browser-panel {onkeydown}>
  {#if access === 'checking'}
    <p class="centre" role="status">Checking file access...</p>
  {:else if access !== 'ready'}
    <p class="denied">{access.denied ?? 'This connection cannot read host files.'}</p>
  {:else}
    <div class="header" class:sheet data-surface-subheader>
      <Tooltip text={loading ? 'Refreshing…' : 'Refresh files'} side="below">
        <IconButton label="Refresh workspace files" icon="refresh-cw" onclick={onrefresh} />
      </Tooltip>
      <input
        bind:this={field}
        type="search"
        name="project-files-search"
        placeholder="Search files"
        aria-label="Search {project} files"
        spellcheck="false"
        autocomplete="off"
        value={query}
        oninput={(event) => {
          onquery(event.currentTarget.value);
        }}
        onkeydown={onFieldKey}
      />
      {#if folds !== 'none'}
        <Tooltip text={foldWords} side="below">
          <IconButton label={foldWords} icon={folds === 'open' ? 'chevrons-down-up' : 'chevrons-up-down'} onclick={() => onfold?.()} />
        </Tooltip>
      {/if}
    </div>
    {#if error !== undefined}
      <button type="button" class="error" onclick={() => onretry?.()}>{error} Click to retry.</button>
    {/if}
    {#if more}<p class="line">More matches available. Refine your search.</p>{/if}
    <p class="line" class:said={listed || !loading} role="status">{loading ? 'Loading files…' : ''}</p>
    {#if noMatch}
      <p class="line none">No matching files.</p>
    {:else}
      <div class="tree">{@render tree()}</div>
    {/if}
  {/if}
</div>

<style>
  .files {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-block-size: 0;
    background: var(--surface-base);
    font-size: var(--code-text-meta);
  }

  /* Beside the chat the header is 36px with 4px under it and no line; in a sheet it is 40px with a line (0.2). */
  .header {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
    block-size: calc(var(--space-8) + var(--space-1));
    margin-block-end: var(--space-1);
    padding-inline: var(--space-2);
    border-block-end: var(--border-thin) solid transparent;
  }

  .header.sheet {
    block-size: var(--space-10);
    margin-block-end: 0;
    border-block-end-color: var(--code-line);
  }

  /* The search field: no border and no fill at rest, a fill on hover, the base surface and the ring with the focus. */
  input {
    flex: 1;
    min-inline-size: 0;
    block-size: var(--code-control-dense);
    padding: 0 var(--space-2);
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: var(--code-text-small);
  }

  input::placeholder {
    color: var(--code-quiet-text);
  }

  input:hover {
    background: var(--surface-hover);
  }

  input:focus-visible {
    background: var(--surface-base);
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .error {
    flex: none;
    inline-size: 100%;
    padding: var(--space-4);
    border: 0;
    background: none;
    color: var(--status-down-text);
    font: inherit;
    line-height: var(--leading-relaxed);
    text-align: start;
  }

  .error:hover {
    background: var(--surface-hover);
  }

  .error:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .line {
    flex: none;
    margin: 0;
    padding: var(--space-1) var(--space-3);
    color: var(--code-quiet-text);
  }

  .none {
    padding-block: var(--space-4);
  }

  /* Said to assistive technology and not shown: the tree already has rows. */
  .said {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    padding: 0;
    overflow: hidden;
    clip-path: inset(50%);
  }

  .tree {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-block-size: 0;
    overflow: auto;
  }

  .centre {
    display: flex;
    flex: 1;
    align-items: center;
    justify-content: center;
    margin: 0;
    color: var(--code-quiet-text);
    font-size: var(--code-text-body);
  }

  .denied {
    margin: 0;
    padding: var(--space-4);
    color: var(--code-quiet-text);
    font-size: var(--code-text-body);
  }
</style>
