<script lang="ts">
  import '../picker.css';
  import CodeIcon from '../../../icons/CodeIcon.svelte';
  import type { ModelsNotice } from '../catalogue';

  interface Props {
    notice: ModelsNotice;
    /** Asks for the models again; left out, a failure has no Retry. */
    onRetry?: (() => void) | undefined;
  }

  const { notice, onRetry }: Props = $props();
</script>

<!-- Why the list is short, on a line of its own under the list with the whole reason, never inside a row. -->
<div class="notice" class:failed={notice.failed} role="status">
  {#if notice.failed}<CodeIcon name="triangle-alert" />{/if}
  <span class="words">{notice.words}</span>
  {#if notice.failed && onRetry !== undefined}<button type="button" class="retry" onclick={onRetry}>Retry</button>{/if}
</div>

<style>
  .notice {
    display: flex;
    gap: var(--space-2);
    align-items: flex-start;
    padding: var(--space-2) var(--space-3);
    border-block-start: var(--border-thin) solid var(--code-composer-picker-line);
    color: var(--code-composer-picker-quiet);
    font-size: var(--code-text-meta);
    line-height: var(--leading-normal);
  }

  .notice.failed {
    color: var(--text);
  }

  .failed :global(.code-icon) {
    flex: none;
    inline-size: var(--code-composer-picker-mark);
    block-size: var(--code-composer-picker-mark);
    margin-block-start: var(--space-1);
    color: var(--status-warning);
  }

  /* The whole reason wraps between words; a long path or code breaks only where it must. */
  .words {
    flex: 1;
    min-inline-size: 0;
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
