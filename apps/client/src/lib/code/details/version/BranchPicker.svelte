<script lang="ts">
  import Switch from '$lib/forms/switch/Switch.svelte';
  import type { Anchor } from '$lib/overlay/helptip/placement';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import { matchingRefs, offersNewRef, refBadge } from '../labels';
  import type { Ref } from '../model';
  import Popup from '../row/Popup.svelte';

  interface Props {
    refs: readonly Ref[];
    /** The name of the ref of the thread: its row has the selected fill. */
    current?: string | undefined;
    /** The first load runs, or a next page loads. */
    loading?: 'first' | 'more' | undefined;
    /** How many refs the repository has, when more exist than the engine gave. */
    total?: number | undefined;
    /** The picker chooses the base of a new worktree: it shows "Start from remote" and offers no new ref. */
    base?: boolean;
    fromRemote?: boolean;
    /** A line of notice, such as "Stop the current turn to switch worktrees." */
    notice?: string | undefined;
    anchor: () => Anchor;
    from?: HTMLElement | undefined;
    onPick: (name: string) => void;
    onCreate?: ((name: string) => void) | undefined;
    onFromRemote?: ((on: boolean) => void) | undefined;
    onClose: (back: boolean) => void;
  }

  const { refs, current, loading, total, base = false, fromRemote = false, notice, anchor, from, onPick, onCreate, onFromRemote, onClose }: Props = $props();
  const id = $props.id();
  let query = $state('');
  let at = $state(0);
  let list = $state<HTMLElement>();
  const shown = $derived(matchingRefs(refs, query));
  const create = $derived(!base && onCreate !== undefined && offersNewRef(refs, query));
  const count = $derived(shown.length + (create ? 1 : 0));
  const status = $derived(loading === 'first' ? 'Loading refs...' : loading === 'more' ? 'Loading more refs...' : total !== undefined && total > refs.length ? `Showing ${String(refs.length)} of ${String(total)} refs` : undefined);

  function pick(index: number): void {
    const ref = shown[index];
    if (ref === undefined && create && index === shown.length) onCreate?.(query.trim());
    else if (ref === undefined || ref.disabled === true) return;
    else onPick(ref.name);
    onClose(true);
  }

  function onkeydown(event: KeyboardEvent): void {
    if (event.isComposing || count === 0) return;
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      at = (at + (event.key === 'ArrowDown' ? 1 : count - 1)) % count;
      list?.querySelector(`#${CSS.escape(`${id}-${String(at)}`)}`)?.scrollIntoView({ block: 'nearest' });
    } else if (event.key === 'Enter') {
      event.preventDefault();
      pick(at);
    }
  }

  const focus = (node: HTMLInputElement): void => node.focus();
</script>

<!-- The picker of a ref (07 section 2.2): a search, the refs in the order of the engine with one mark each, a new ref. -->
<Popup {anchor} {from} fit {onClose}>
  <label class="search">
    <CodeIcon name="search" />
    <input
      type="text"
      role="combobox"
      aria-label="Search refs"
      aria-expanded="true"
      aria-controls="{id}-list"
      aria-autocomplete="list"
      aria-activedescendant={count === 0 ? undefined : `${id}-${String(at)}`}
      placeholder="Search refs..."
      autocomplete="off"
      spellcheck="false"
      bind:value={query}
      oninput={() => (at = 0)}
      {onkeydown}
      {@attach focus}
    />
  </label>
  <div class="refs" id="{id}-list" role="listbox" aria-label="Refs" bind:this={list}>
    {#each shown as ref, index (ref.name)}
      {@const badge = refBadge(ref)}
      <!-- svelte-ignore a11y_click_events_have_key_events (the keys are on the search field, which keeps focus) -->
      <div id="{id}-{index}" class="ref" class:active={index === at} class:current={ref.name === current} role="option" tabindex="-1" aria-selected={ref.name === current} aria-disabled={ref.disabled === true} onclick={() => pick(index)} onpointermove={() => (at = index)}>
        <span class="details-label">{ref.name}</span>
        {#if badge !== undefined}<span class="details-trail">{badge}</span>{/if}
      </div>
    {/each}
    {#if create}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div id="{id}-{shown.length}" class="ref" class:active={at === shown.length} role="option" tabindex="-1" aria-selected="false" onclick={() => pick(shown.length)} onpointermove={() => (at = shown.length)}>
        <span class="details-label">Create new ref "{query.trim()}"</span>
      </div>
    {/if}
    {#if count === 0 && loading !== 'first'}<p class="details-note empty">No refs found.</p>{/if}
  </div>
  {#if base}
    <div class="row">
      <span class="words" id="{id}-remote"><CodeIcon name="refresh-cw" /> Start from remote</span>
      <Switch checked={fromRemote} label="Start worktree from remote" disabled={onFromRemote === undefined} onchange={(on) => onFromRemote?.(on)} />
    </div>
  {/if}
  {#if notice !== undefined}<p class="details-note status">{notice}</p>{/if}
  <p class="details-note status" role="status">{status ?? ''}</p>
</Popup>

<style>
  .search {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
    margin: var(--details-row-pad) var(--space-3) 0;
    padding-block-end: var(--details-step);
    border-block-end: var(--border-thin) solid var(--code-line);
    color: var(--code-quiet-text);
  }

  .search:focus-within {
    border-block-end-color: var(--focus-ring);
  }

  input {
    flex: 1;
    min-inline-size: 0;
    padding: 0;
    border: 0;
    outline: 0;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: var(--code-text-body);
  }

  .refs {
    flex: 1;
    max-block-size: var(--details-refs);
    padding: var(--space-2) var(--space-1);
    overflow-y: auto;
  }

  .ref {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-block-size: var(--code-menu-item);
    padding: var(--space-1) var(--space-2);
    border-radius: var(--radius-sm);
    font-size: var(--code-text-body);
  }

  .ref.current {
    background: var(--surface-selected);
  }

  .ref.active {
    background: var(--surface-hover);
    outline: var(--border-thin) solid var(--border-strong);
  }

  .ref[aria-disabled='true'] {
    opacity: var(--opacity-disabled);
  }

  .empty {
    text-align: center;
  }

  .row {
    display: flex;
    flex: none;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    border-block-start: var(--border-thin) solid var(--code-line);
  }

  .words {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    font-weight: var(--weight-medium);
  }

  .status {
    flex: none;
    padding: 0 var(--space-3);
    font-weight: var(--weight-medium);
  }

  .status:not(:empty) {
    padding-block: var(--space-2);
  }
</style>
