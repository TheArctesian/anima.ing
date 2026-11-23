<script lang="ts">
  import LineChart from '$lib/charts/LineChart.svelte';
  import type { TimeWindow } from '$lib/charts/scales';
  import type { MetricRange } from '$lib/metrics/api';
  import { tileFigures, type Chart } from '$lib/metrics/overview-charts';
  import { rangeGap } from '$lib/metrics/ranges';

  const { chart, span, range, phrase }: { chart: Chart; span: TimeWindow; range: MetricRange; phrase: string } = $props();
  const figures = $derived(tileFigures(chart));
</script>

<div class="cell" class:percent={chart.axis === 'percent'}>
  <h3>{chart.title}</h3>
  <LineChart quiet title={`${chart.title}, ${phrase}`} gap={rangeGap(range)} series={chart.series} {span} axis={chart.axis} format={chart.format} threshold={chart.threshold} />
  <div class="caption">
    {#if figures}<span>Range peak · {figures.peak}</span>{/if}
    {#if chart.threshold !== undefined}<span class="guide">Warning guide · {chart.format(chart.threshold)}</span>{/if}
  </div>
</div>

<style>
  .cell { min-inline-size: 0; }

  .percent { --chart-value-gutter: var(--space-12); }

  @media (max-width: 600px) {
    .cell { --chart-plot-height: calc(var(--space-16) + var(--space-6)); }
  }

  h3 {
    margin: 0 0 var(--space-2);
    font-size: var(--text-md);
    font-weight: var(--weight-medium);
  }

  .caption {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: var(--space-1) var(--space-3);
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  .guide { margin-inline-start: auto; }
</style>
