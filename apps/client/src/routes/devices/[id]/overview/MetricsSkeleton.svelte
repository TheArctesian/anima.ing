<script lang="ts">
  import { untrack } from 'svelte';
  import ChartSkeleton from '$lib/charts/ChartSkeleton.svelte';
  import type { SeriesColour } from '$lib/charts/plot';
  import type { MetricRange } from '$lib/metrics/api';
  import { DEGRADED_ABOVE, formatPercent } from '$lib/metrics/units';
  import { rangeWindow } from '$lib/metrics/ranges';
  import './geometry.css';

  interface Props {
    range: MetricRange;
    now: () => Date;
    /** The series count of each chart in the device's last answer; `null` before any, which draws the CPU shape. */
    cells?: readonly number[] | null;
  }

  const { range, now, cells = null }: Props = $props();
  // The window is fixed when the range is: a skeleton is not redrawn with the clock.
  const span = $derived(rangeWindow(range, untrack(now)));
  const series = $derived(cells?.[0] ?? 1);
  /** A chart with several series has a legend row; the placeholder reserves it with blank entries. */
  const legendOf = (series: number) => (series > 1 ? Array.from({ length: series }, (_, index) => ({ label: '\u00a0', colour: ((index % 6) + 1) as SeriesColour })) : []);
</script>

<div class="chart-grid" aria-hidden="true">
  <div class="cell">
    <p class="chart-title"></p>
    <ChartSkeleton {span} axis="percent" format={formatPercent} threshold={DEGRADED_ABOVE} quiet legend={legendOf(series)} />
  </div>
</div>
