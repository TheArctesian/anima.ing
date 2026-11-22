<script lang="ts">
  import { shortCount } from '../patch';

  interface Props {
    additions: number;
    deletions: number;
    /** Two columns of equal width, for a list of file headers. */
    aligned?: boolean;
  }

  const { additions, deletions, aligned = false }: Props = $props();
</script>

<!-- The counts of a diff (report 05, Section 1.5): added lines, then removed lines, in short numbers. -->
<span class="stat" class:aligned role="img" aria-label="{additions} additions, {deletions} deletions">
  <span class="up" aria-hidden="true">+{shortCount(additions)}</span>
  <span class="down" aria-hidden="true">-{shortCount(deletions)}</span>
</span>

<style>
  .stat {
    display: inline-flex;
    flex: none;
    gap: var(--space-1);
    font-family: var(--font-mono);
    font-size: var(--code-text-meta);
    font-variant-numeric: tabular-nums;
  }

  .aligned {
    display: inline-grid;
    grid-template-columns: 4ch 4ch;
    gap: var(--space-2);
    text-align: end;
  }

  .up {
    color: var(--status-up-text);
  }

  .down {
    color: var(--status-down-text);
  }
</style>
