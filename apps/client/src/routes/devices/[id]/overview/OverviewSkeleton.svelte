<script lang="ts">
  import type { MetricsSource } from '$lib/hub/generated/graphql';
  import type { MetricRange } from '$lib/metrics/api';
  import { hasMetrics } from '$lib/metrics/source/has-metrics';
  import './geometry.css';
  import MetricsSkeleton from './MetricsSkeleton.svelte';

  const { range, source, now }: { range: MetricRange; source: MetricsSource; now: () => Date } = $props();
</script>

<div class="overview-fit" aria-hidden="true">
  <div class="health"></div>
  <div class="overview-card metrics">
    <div class="overview-card-title"></div>
    {#if hasMetrics(source)}
      <div class="readings"></div>
      <MetricsSkeleton {range} {now} />
    {:else}
      <div class="empty"></div>
    {/if}
  </div>
</div>

<style>
  .health { min-block-size: var(--target-min-touch); }

  .metrics {
    padding: var(--space-3);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-lg);
    background: var(--surface-base);
  }

  .readings,
  .empty {
    min-block-size: var(--target-min-touch);
    border-radius: var(--radius-md);
    background: var(--surface-sunken);
  }
</style>
