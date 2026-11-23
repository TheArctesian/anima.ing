<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    label?: string | undefined;
    children: Snippet;
  }

  const { label, children }: Props = $props();
</script>

<ul aria-label={label}>{@render children()}</ul>

<style>
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    border-block-start: var(--border-thin) solid var(--border-subtle);
  }
</style>
