<script lang="ts">
  interface Props {
    /** Why the last call did not go. */
    words: string;
    /** Sends the message again; left out, the line has no Retry (a failure that is not a send). */
    onRetry?: (() => void) | undefined;
    /** A call is on its way: Retry waits. */
    busy: boolean;
  }

  const { words, onRetry, busy }: Props = $props();
</script>

<!-- A send that did not go keeps its text in the editor, says why, and can go again (TH97). -->
<div class="problem">
  <p role="alert">{words}</p>
  {#if onRetry !== undefined}<button type="button" class="retry" disabled={busy} onclick={onRetry}>Retry</button>{/if}
</div>

<style>
  .problem {
    display: flex;
    gap: var(--space-2);
    align-items: flex-start;
    padding: 0 var(--code-composer-pad-inline) var(--space-2);
    font-size: var(--code-text-meta);
  }

  .problem p {
    flex: 1;
    min-inline-size: 0;
    margin: 0;
    color: var(--status-down-text);
    overflow-wrap: break-word;
  }

  .retry {
    flex: none;
    padding: var(--space-1) var(--space-2);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: none;
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }

  .retry:hover {
    background: var(--surface-hover);
  }

  .retry:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: var(--border-focus);
  }
</style>
