<script lang="ts">
  import IconButton from '../kit/IconButton.svelte';
  import './details.css';

  /** What the search of the thread knows now. */
  export type FindState = { readonly kind: 'idle' } | { readonly kind: 'loading' } | { readonly kind: 'failed' } | { readonly kind: 'matches'; readonly index: number; readonly total: number; readonly counting?: boolean };

  interface Props {
    query: string;
    find: FindState;
    onQuery: (query: string) => void;
    onNext: () => void;
    onPrevious: () => void;
    onRetry: () => void;
    onClose: () => void;
    input?: HTMLInputElement | undefined;
    /** What is searched, as the name of the field: the thread, or the page of a browser tab. */
    words?: string;
  }

  let { query, find, onQuery, onNext, onPrevious, onRetry, onClose, input = $bindable(), words = 'Find in thread' }: Props = $props();
  const none = $derived(find.kind === 'matches' && find.total === 0);
  const count = $derived.by(() => {
    if (query === '' || find.kind === 'idle') return '';
    if (find.kind === 'loading') return 'Searching…';
    if (find.kind === 'failed') return 'Search failed';
    return find.counting === true ? `${String(find.index)}/…` : `${String(find.index)}/${String(find.total)}`;
  });
  const still = $derived(find.kind !== 'matches' || find.total === 0 || find.counting === true);

  function onkeydown(event: KeyboardEvent): void {
    if (event.isComposing) return;
    if (event.key === 'Enter') {
      event.preventDefault();
      if (event.shiftKey) onPrevious();
      else onNext();
    } else if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
    }
  }
</script>

<!-- "Find in thread" (07 section 9): it takes the place of the details card while it is open. -->
<div class="find" role="search" aria-label={words} aria-busy={find.kind === 'loading' || (find.kind === 'matches' && find.counting === true)}>
  <input type="search" aria-label={words} placeholder={words} maxlength="200" autocomplete="off" spellcheck="false" value={query} bind:this={input} oninput={(event) => onQuery(event.currentTarget.value)} {onkeydown} />
  <span class="count" class:none aria-live="polite">{count}</span>
  {#if find.kind === 'failed'}<button type="button" class="retry" onclick={onRetry}>Retry</button>{/if}
  <IconButton label="Previous match" icon="chevron-up" disabled={still} onclick={onPrevious} />
  <IconButton label="Next match" icon="chevron-down" disabled={still} onclick={onNext} />
  <IconButton label="Close find" icon="x" onclick={onClose} />
</div>

<style>
  .find {
    display: flex;
    align-items: center;
    gap: var(--details-hair);
    inline-size: 100%;
    block-size: var(--details-find);
    padding-inline: var(--space-3) var(--space-1);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-lg);
    background: var(--surface-overlay);
    box-shadow: var(--elevation-2);
  }

  .find:focus-within {
    border-color: var(--focus-ring);
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

  .count {
    flex: none;
    min-inline-size: var(--space-10);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    font-variant-numeric: tabular-nums;
    text-align: center;
  }

  .count.none {
    color: var(--danger);
  }

  .retry {
    flex: none;
    padding: 0 var(--space-2);
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--text);
    font: inherit;
    font-size: var(--code-text-meta);
    font-weight: var(--weight-medium);
  }

  .retry:hover {
    background: var(--surface-hover);
  }

  .retry:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
  }
</style>
