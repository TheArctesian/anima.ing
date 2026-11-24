<script lang="ts">
  import { contextHubIsHosted } from '$lib/hosted/context';
  import type { MetricsSource } from '$lib/hub/generated/graphql';
  import Icon from '$lib/icons/Icon.svelte';
  import { hasMetrics } from '$lib/metrics/source/has-metrics';
  import { chartsOf } from '$lib/metrics/overview-charts';
  import type { MetricRange } from '$lib/metrics/api';
  import { skeletonDelay } from '$lib/motion/loading/delay.svelte';
  import type { LoadingClock } from '$lib/motion/loading/loading';
  import { press } from '$lib/motion/roles/press';
  import { crossFade } from '$lib/motion/roles/transitions';
  import { RANGES } from '$lib/metrics/ranges';
  import './geometry.css';
  import type { DeviceMetrics } from './metrics.svelte';
  import ResourceReadings from './trends/ResourceReadings.svelte';
  import TrendChart from './trends/TrendChart.svelte';
  import MetricsSkeleton from './MetricsSkeleton.svelte';
  import { metricsOffText, noChartsText } from './overview';

  interface Props {
    source: MetricsSource;
    metrics: Pick<DeviceMetrics, 'view' | 'span' | 'shownRange' | 'chartShape' | 'readings' | 'battery' | 'error' | 'liveError'> &
      Partial<Pick<DeviceMetrics, 'memoryBytes'>>;
    range: MetricRange;
    now: () => Date;
    clock?: LoadingClock;
    onRange: (range: MetricRange) => void;
    onRetry: () => void;
    onSettings: () => void;
    /** The hub's own device: its bars are those of its container (HMD-073). */
    container?: boolean;
    /** The reason line of a daemon that waits for the person, in place of the wait; `null` for none (R-322). */
    needLine?: string | null;
    storageReading?: { path: string; percent: number } | null;
  }

  const { source, metrics, range, now, clock, onRange, onRetry, onSettings, container = false, needLine = null, storageReading = null }: Props = $props();
  // A hosted hub reads no metrics over SSH (HMD-049 is not built), so its sentence names the daemon only (R-222).
  const hosted = contextHubIsHosted();
  let selected = $state<string>();
  let expanded = $state(false);
  const charts = $derived(metrics.view === null ? [] : chartsOf(metrics.view.series, metrics.view.gpus));
  const readings = $derived(metrics.readings !== null && storageReading !== null
    ? { ...metrics.readings, diskPercent: storageReading.percent, diskMount: storageReading.path }
    : metrics.readings);
  const primary = $derived(charts.find((chart) => chart.key === selected) ?? charts[0]);
  // The old chart stays while a switch is on its way, so its label and gap follow the data on screen, not the button.
  const drawn = $derived(metrics.shownRange ?? range);
  const phrase = $derived(RANGES.find((each) => each.range === drawn)?.phrase ?? '');
  const waiting = $derived(metrics.view === null && metrics.error === null);
  // The clock is fixed for the life of the section.
  // svelte-ignore state_referenced_locally
  const skeleton = skeletonDelay(() => waiting, clock);
</script>

<section class="overview-card metrics" aria-labelledby="overview-metrics">
  <div class="head">
    <h2 id="overview-metrics" class="overview-card-title"><Icon name="carbon:chart-line" />Metrics</h2>
    {#if hasMetrics(source)}
      <div class="range" role="group" aria-label="Time range">
        {#each RANGES as choice (choice.range)}
          <button type="button" aria-pressed={choice.range === range} onclick={() => { onRange(choice.range); }} {@attach press()}>
            {choice.label}
          </button>
        {/each}
      </div>
    {/if}
  </div>
  {#if !hasMetrics(source)}
    <!-- D-7: an empty state with its icon, the sentence and the way to turn metrics on. -->
    <div class="metrics-off">
      <span class="off-icon"><Icon name="carbon:chart-line" size="lg" /></span>
      <p>{metricsOffText(hosted)}</p>
      <button type="button" class="settings" onclick={onSettings} {@attach press()}><Icon name="carbon:settings-adjust" size="sm" />Open Settings</button>
    </div>
  {:else}
    <ResourceReadings {readings} {source} battery={metrics.battery} {container} memoryBytes={metrics.memoryBytes ?? null} {charts} active={primary?.key} onSelect={(key) => { selected = key; }} />
    {#if metrics.liveError !== null}
      <p class="live" role="status">{metrics.liveError.message}</p>
    {/if}
    <!-- The skeleton and the charts share one cell, so the cross-fade moves nothing (MOT-007). -->
    <div class="area">
      {#if metrics.error !== null}
        <div class="notice" role="alert" transition:crossFade>
          <p>{metrics.error.message}</p>
          <button type="button" onclick={onRetry} {@attach press()}>Retry</button>
        </div>
      {:else if skeleton.visible}
        <div class="layer" transition:crossFade><MetricsSkeleton {range} {now} cells={metrics.chartShape} /></div>
      {:else if !waiting}
        <div class="layer" transition:crossFade>
          {#if primary === undefined || metrics.span === null}
            <p class="waiting">{noChartsText(metrics.readings !== null, phrase, needLine)}</p>
          {:else}
            {@const span = metrics.span}
            <div class="trend" role="group" aria-label="Selected resource trend">
              <TrendChart chart={primary} {span} range={drawn} {phrase} />
            </div>
            {#if charts.length > 1}
              <details bind:open={expanded}>
                <summary>All resource charts</summary>
                {#if expanded}
                  <div class="chart-grid">
                    {#each charts as chart (chart.key)}
                      <TrendChart {chart} {span} range={drawn} {phrase} />
                    {/each}
                  </div>
                {/if}
              </details>
            {/if}
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</section>

<style>
  .metrics {
    gap: var(--space-3);
    padding: var(--space-3);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-lg);
    background: var(--surface-base);
  }

  @media (max-width: 600px) {
    .metrics { gap: var(--space-1); padding: var(--space-2); }
    .trend { padding-block-start: var(--space-1); }
  }

  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
  }

  .range {
    display: flex;
    gap: 0;
  }

  .range button {
    min-block-size: var(--target-min-touch);
    border-color: transparent;
    background: transparent;
    color: var(--text-secondary);
  }

  .range button[aria-pressed='true'] {
    border-color: transparent;
    background: var(--surface-selected);
    color: var(--text);
    text-decoration: none;
    font-weight: var(--weight-medium);
  }

  .area {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    min-inline-size: 0;
  }

  .layer,
  .notice {
    grid-area: 1 / 1;
    min-inline-size: 0;
  }

  details > .chart-grid { margin-block-end: var(--space-3); }

  .trend {
    border-block-start: var(--border-thin) solid var(--border-subtle);
    padding-block-start: var(--space-3);
  }

  summary {
    align-content: center;
    min-block-size: var(--target-min-touch);
    color: var(--text-secondary);
    cursor: pointer;
  }

  p { margin: 0; }

  .waiting,
  .live {
    color: var(--text-secondary);
  }

  .notice p {
    margin-block-end: var(--space-2);
  }

  button {
    box-sizing: border-box;
    min-block-size: var(--target-min-desktop);
    padding: 0 var(--space-3);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    background: var(--surface-raised);
    color: var(--text);
    font: inherit;
  }

  /* The chosen range has a colour, an underline and, for more contrast, a ring: never colour alone. */
  button[aria-pressed='true'] {
    border-color: var(--accent);
    background: var(--accent-tint);
    text-decoration: underline;
    text-decoration-thickness: var(--border-focus);
    text-underline-offset: var(--space-1);
  }

  @media (prefers-contrast: more) {
    button[aria-pressed='true'] {
      box-shadow: inset 0 0 0 var(--border-focus) currentcolor;
    }
  }

  .off-icon {
    color: var(--text-secondary);
    font-size: var(--text-2xl);
    line-height: var(--leading-tight);
  }

  .metrics-off p {
    color: var(--text-secondary);
  }

  .settings {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    min-block-size: var(--target-min-touch);
    padding: 0 var(--space-4);
    transition: background-color var(--duration-fast) var(--ease-enter);
  }

  .settings:hover {
    background: var(--surface-hover);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--accent);
    outline-offset: var(--border-focus);
  }
</style>
