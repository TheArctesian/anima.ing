<script lang="ts">
  import CodeIcon from '../icons/CodeIcon.svelte';

  interface Props {
    /** Why the chat takes no message: "The program of this chat ended". */
    status: string;
  }

  /* Stands in place of the composer for a chat that takes no message (03 5.7): a 48 px bar, "Read-only" and why. */
  const { status }: Props = $props();
</script>

<div class="bar">
  <span class="lead"><CodeIcon name="eye" /> <strong>Read-only</strong></span>
  <span class="status" role="status">{status}</span>
</div>

<style>
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
