<script lang="ts">
  import type { MetricsSource } from '$lib/hub/generated/graphql';
  import type { BatteryBar } from '$lib/metrics/battery/battery';
  import { barRow, type Bar, type BarReadings } from '$lib/metrics/bars/bars';
  import { containerRow } from '$lib/metrics/container/container';
  import type { Chart } from '$lib/metrics/overview-charts';
  import { press } from '$lib/motion/roles/press';

  interface Props {
    readings: BarReadings | null;
    source: MetricsSource;
    battery: BatteryBar | null;
    container: boolean;
    memoryBytes: number | null;
    charts: readonly Chart[];
    active: string | undefined;
    onSelect: (key: string) => void;
  }

  const { readings, source, battery, container, memoryBytes, charts, active, onSelect }: Props = $props();
  const row = $derived(container ? containerRow(readings, source, memoryBytes) : barRow(readings, source, battery));
  const sample = $derived(readings?.at === undefined ? null : new Date(readings.at));

  function keyFor(bar: Bar): string {
    if (bar.key === 'disk') {
      if (readings?.diskMount != null) return `disk:${readings.diskMount}`;
      const disks = charts.filter((chart) => chart.key.startsWith('disk:'));
      return disks.length === 1 ? disks[0]?.key ?? '' : '';
    }
    if (bar.key === 'vram') return `gpu:${String(readings?.vramGpu?.index)}:vram`;
    return bar.key;
  }
</script>

{#snippet value(bar: Bar)}
  <span>{bar.label}{#if bar.note} <small>{bar.note}</small>{/if}</span>
  <strong>{bar.text}{#if bar.degraded} <span aria-label="Above warning threshold">▲</span>{/if}</strong>
{/snippet}

{#if row.kind === 'bars'}
  <div class="snapshot">
    <span>Latest readings</span>
    {#if sample && Number.isFinite(sample.getTime())}
      <time datetime={sample.toISOString()}>{sample.toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' })}</time>
    {/if}
  </div>
  <div class="resources" role="group" aria-label="Latest resource readings">
    {#each row.bars as bar (bar.key)}
      {@const key = keyFor(bar)}
      {#if charts.some((chart) => chart.key === key)}
        <button type="button" class="reading" class:degraded={bar.degraded} title={bar.tooltip} aria-pressed={active === key} aria-label={`${bar.label}: ${bar.spoken ?? bar.text}`} onclick={() => onSelect(key)} {@attach press()}>
          {@render value(bar)}
        </button>
      {:else}
        <div class="reading" class:degraded={bar.degraded} title={bar.tooltip}
          role={bar.percent === null ? undefined : 'meter'}
          aria-label={bar.percent === null ? undefined : bar.label}
          aria-valuemin={bar.percent === null ? undefined : 0}
          aria-valuemax={bar.percent === null ? undefined : 100}
          aria-valuenow={bar.percent ?? undefined}
          aria-valuetext={bar.percent === null ? undefined : (bar.spoken ?? bar.text)}>
          {@render value(bar)}
        </div>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .snapshot {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: var(--space-1) var(--space-3);
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  .resources {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, calc(var(--space-16) + var(--space-2))), 1fr));
    gap: var(--space-2);
  }

  .reading {
    display: grid;
    align-content: center;
    min-block-size: var(--target-min-touch);
    min-inline-size: 0;
    padding: var(--space-1) var(--space-2);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
    background: var(--surface-sunken);
    color: var(--text-secondary);
    font: inherit;
    font-size: var(--text-sm);
    text-align: start;
    overflow-wrap: anywhere;
  }

  strong { color: var(--text); font-variant-numeric: tabular-nums; }
  small { color: var(--text-secondary); font-size: var(--text-xs); }
  .degraded strong { color: var(--status-degraded-text); }

  button[aria-pressed='true'] {
    border-color: var(--accent);
    background: var(--surface-selected);
    color: var(--text);
  }

  button:focus-visible { outline: var(--border-focus) solid var(--focus-ring); outline-offset: var(--border-focus); }
</style>
