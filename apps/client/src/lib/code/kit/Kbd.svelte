<script lang="ts">
  interface Props {
    /** The keys as written: "⌘K", "Esc", "⇧⌘O". */
    keys: string;
    /** `outline` draws a line in place of the fill, for a key shown on a filled surface. */
    outline?: boolean;
  }

  const { keys, outline = false }: Props = $props();
</script>

<!-- A key or a chord (02 section 3.4): 20 px high, 12 px text. -->
<kbd class:outline>{keys}</kbd>

<style>
  kbd {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    block-size: var(--code-kbd);
    min-inline-size: var(--code-kbd);
    padding: 0 var(--space-1);
    border-radius: var(--radius-sm);
    background: var(--surface-sunken);
    color: var(--text-secondary);
    font-family: var(--font-ui);
    font-size: var(--text-xs);
    font-weight: var(--weight-medium);
    line-height: 1;
    white-space: nowrap;
  }

  .outline {
    padding: 0 var(--space-2);
    border: var(--border-thin) solid var(--border-strong);
    border-block-end-width: calc(var(--border-thin) * 2);
    background: none;
  }
</style>
