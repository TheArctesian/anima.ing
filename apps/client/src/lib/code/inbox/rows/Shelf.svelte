<script lang="ts">
  import CodeIcon from '../../icons/CodeIcon.svelte';

  interface Props {
    title: string;
    count: number;
    open: boolean;
    /** The first shelf: margin-top auto, so the shelves sit at the foot of a short list (02 section 5.4). */
    first: boolean;
    /** The ID of the list it opens. */
    controls: string;
    onToggle: () => void;
  }

  const { title, count, open, first, controls, onToggle }: Props = $props();
</script>

<!-- A shelf header (02 section 5.6): 32 px, label, count, chevron turned -90° while folded. -->
<li class="shelf" class:first>
  <button type="button" aria-expanded={open} aria-controls={controls} onclick={onToggle}>
    <span>{title}</span>
    <span class="count">{count}</span>
    <span class="chevron" class:folded={!open}><CodeIcon name="chevron-down" /></span>
  </button>
</li>

<style>
  .shelf {
    block-size: var(--code-shelf);
    margin: 0 var(--border-focus);
    list-style: none;
  }

  .first {
    margin-block-start: auto;
  }

  button {
    display: flex;
    gap: var(--space-2);
    align-items: center;
    inline-size: 100%;
    block-size: 100%;
    padding: 0 var(--space-2);
    border: 0;
    border-radius: var(--radius-md);
    background: none;
    color: var(--code-quiet-text);
    font: inherit;
    font-size: var(--code-text-meta);
    font-weight: var(--weight-medium);
    cursor: pointer;
  }

  button:focus-visible {
    outline: none;
    box-shadow: inset 0 0 0 var(--border-focus) var(--focus-ring);
  }

  .count {
    margin-inline-start: auto;
    font-weight: var(--weight-regular);
    font-variant-numeric: tabular-nums;
  }

  .chevron {
    display: inline-flex;
    /* A spring token: zero under reduced motion, from the media query or the Settings switch (09 finding 27). */
    transition: transform var(--spring-snappy);
  }

  .chevron :global(.code-icon) {
    inline-size: var(--text-xs);
    block-size: var(--text-xs);
  }

  .folded {
    transform: rotate(-90deg);
  }
</style>
