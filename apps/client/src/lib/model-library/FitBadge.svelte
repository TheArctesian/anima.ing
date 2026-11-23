<script lang="ts">
  import type { ModelFit } from './api';
  import { fitBadge } from './fit';

  /** FIT-005: the fit of a variant in words, with the wash of its tone; the words alone say it (MAT-005). */
  const { fit }: { fit: ModelFit | null } = $props();
  const badge = $derived(fit === null ? { words: 'No machine', tone: 'unknown' as const } : fitBadge(fit));
</script>

<span class="badge {badge.tone}">{badge.words}</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    block-size: var(--space-5);
    padding-inline: var(--space-2);
    border-radius: var(--radius-sm);
    background: var(--status-unknown-tint);
    color: var(--text-secondary);
    font-size: var(--text-xs);
    white-space: nowrap;
  }

  .up {
    background: var(--status-up-tint);
    color: var(--status-up-text);
  }

  .degraded {
    background: var(--status-degraded-tint);
    color: var(--status-degraded-text);
  }

  .down {
    background: var(--status-down-tint);
    color: var(--status-down-text);
  }
</style>
