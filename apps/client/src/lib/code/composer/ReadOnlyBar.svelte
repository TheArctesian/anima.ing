<script lang="ts">
  import type { Snippet } from 'svelte';
  import CodeIcon from '../icons/CodeIcon.svelte';

  interface Props {
    /** Why the chat takes no message: "The program of this chat ended". */
    status: string;
    /** Controls at the end of the bar, when the page has some for a chat that is only read. */
    actions?: Snippet;
  }

  /*
   * Stands in place of the composer for a chat that takes no message (03 5.7): a 48 px bar, "Read-only" and why. In a
   * composer under 560 px the reason drops to a line of its own (T3 Code `ReadOnlyComposerBar.tsx:15-35`).
   */
  const { status, actions }: Props = $props();
</script>

<div class="surface">
  <div class="bar">
    <div class="words">
      <span class="lead"><CodeIcon name="eye" /> <strong>Read-only</strong></span>
      <span class="status" role="status">{status}</span>
    </div>
    {@render actions?.()}
  </div>
</div>

<style>
  .surface {
    container-type: inline-size;
  }

  .words {
    display: flex;
    flex: 1;
    gap: var(--space-3);
    align-items: center;
    min-inline-size: 0;
  }

  @container (width <= 35rem) {
    .words {
      flex-direction: column;
      gap: calc(var(--space-1) / 2);
      align-items: flex-start;
    }

    .status {
      max-inline-size: 100%;
      font-size: var(--code-text-meta);
    }
  }

  .bar {
    display: flex;
    gap: var(--space-3);
    align-items: center;
    min-block-size: var(--space-12);
    padding-block: var(--space-2);
    padding-inline: var(--space-5) var(--space-2);
    border: var(--border-thin) solid var(--code-composer-line);
    border-radius: var(--code-composer-radius);
    background: var(--code-composer-surface);
    font-size: var(--code-text-body);
  }

  .lead {
    display: inline-flex;
    flex: none;
    gap: var(--space-2);
    align-items: center;
    color: var(--text-secondary);
  }

  strong {
    color: var(--text);
    font-weight: var(--weight-medium);
  }

  .status {
    min-inline-size: 0;
    overflow: hidden;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
