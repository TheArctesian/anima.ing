<script lang="ts">
  interface Props {
    label: string;
    rows?: number;
  }

  const { label, rows = 3 }: Props = $props();
  const lines = $derived(Array.from({ length: rows }, (_, index) => index));
</script>

<div class="skeleton" role="status" aria-label={label} aria-busy="true">
  {#each lines as line (line)}<span class="row"></span>{/each}
</div>

<style>
  .skeleton {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-4);
  }

  .row {
    display: block;
    block-size: var(--space-4);
    border-radius: var(--radius-sm);
    background: var(--surface-hover);
  }

  .row:nth-child(even) {
    inline-size: 75%;
  }
</style>
