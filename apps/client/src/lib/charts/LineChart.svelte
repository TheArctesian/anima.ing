<script lang="ts">
  import { animate } from '$lib/motion/animate';
  import { springMotion } from '$lib/motion/options';
  import { motionPreference, type MotionPreference } from '$lib/motion/preference';
  import { crossFade } from '$lib/motion/roles/transitions';
  import ChartFrame from './ChartFrame.svelte';
  import { chartLabel } from './label';
  import { highestOf, seriesPattern, seriesPlot, type ChartSeries } from './plot';
  import { PLOT, slideOffset, lengthOf, valueScale, type TimeWindow, type ValueAxis } from './scales';

  interface Props {
    title: string;
    series: readonly ChartSeries[];
    span: TimeWindow;
    axis: ValueAxis;
    format: (value: number) => string;
    threshold?: number;
    /** A wider gap between two points, in the `at` unit, breaks the line. */
    gap?: number;
    timeAxis?: boolean;
    quiet?: boolean;
    empty?: string;
    preference?: MotionPreference;
  }

  const {
    title,
    series,
    span,
    axis,
    format,
    threshold,
    gap,
    timeAxis = true,
    quiet = false,
    empty,
    preference = motionPreference(),
  }: Props = $props();
  const peak = $derived(Math.max(0, ...series.map((one) => highestOf(one.points))));
  const scale = $derived(valueScale(axis, peak));
  const visibleThreshold = $derived(quiet && threshold !== undefined && peak <= threshold ? undefined : threshold);
  const plots = $derived(series.map((one) => ({ ...one, ...seriesPlot(one.points, span, scale, gap) })));
  const label = $derived(chartLabel({ title, series, top: scale.top, format, threshold: visibleThreshold, empty }));
  const length = $derived(lengthOf(span));
  let lines = $state<SVGGElement>();
  let before: TimeWindow | undefined;

  /** The lines start offset by how far the time axis moved and spring to rest; only `translate` moves (5.7.3, MOT-002, MOT-006, MOT-041). */
  $effect(() => {
    const now = span;
    const was = before;
    before = now;
    const offset = was === undefined ? 0 : slideOffset(was, now);
    // `bind:this` clears to null when the chart goes away.
    if (!lines || offset === 0) return;
    const shown = Number.parseFloat(getComputedStyle(lines).translate.split(' ')[0] ?? '') || 0;
    animate(lines, { translate: { x: shown + offset } }, { type: 'instant' });
    animate(lines, { translate: { x: 0 } }, springMotion('default', preference.reduced));
  });
</script>

<div class="chart" class:quiet>
  {#key length}
    <div class="layer" transition:crossFade={{ preference }}>
      <ChartFrame {span} {scale} {format} threshold={visibleThreshold} {timeAxis} {quiet} legend={series} {label}>
        <svg class="lines" aria-hidden="true" viewBox="0 0 {PLOT.width} {PLOT.height}" preserveAspectRatio="none">
          <g bind:this={lines}>
            {#each plots as plot, index (index)}
              <g
                style:--series="var(--chart-series-{plot.colour})"
                class:dash={seriesPattern(plot.colour) === 'dash'}
                class:dot={seriesPattern(plot.colour) === 'dot'}
              >
                {#each plot.bands as points, band (band)}<polygon {points} />{/each}
                {#each plot.lines as points, run (run)}<polyline {points} />{/each}
              </g>
            {/each}
          </g>
        </svg>
      </ChartFrame>
    </div>
  {/key}
</div>

<style>
  /* A range change cross-fades two layers in one cell, so nothing moves while they swap (MOT-007). */
  .chart {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    min-inline-size: 0;
  }

  .layer {
    grid-area: 1 / 1;
    min-inline-size: 0;
  }

  .lines {
    position: absolute;
    inset: 0;
    inline-size: 100%;
    block-size: 100%;
    overflow: hidden;
  }

  polyline {
    fill: none;
    stroke: var(--series);
    stroke-width: var(--border-focus);
    stroke-linejoin: round;
    vector-effect: non-scaling-stroke;
  }

  /* A second and a third pattern beside the colour (UI-003); the lengths scale with the text, the stroke does not. */
  .dash polyline {
    stroke-dasharray: var(--space-2) var(--space-1);
  }

  .dot polyline {
    stroke-dasharray: var(--border-focus) var(--space-1);
  }

  .quiet polygon { opacity: var(--opacity-disabled); }

  polygon {
    fill: var(--series);
    fill-opacity: var(--opacity-band);
    stroke: none;
  }
</style>
