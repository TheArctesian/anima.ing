import type { MetricsSource } from '$lib/hub/generated/graphql';
import { percentText, type Bar, type BarReadings, type BarRow } from '../bars/bars';
import { hasMetrics } from '../source/has-metrics';
import { DEGRADED_ABOVE } from '../units';

/** K1 gives no memory limit: it is found from the bytes in use and their percent (HMD-071, K8 Section 4). */

const KIND = 'MEMORY_BYTES';
const IEC = ['B', 'KiB', 'MiB', 'GiB', 'TiB'] as const;
const MIB = 1024 ** 2;

interface Sample {
  readonly kind: string;
  readonly value: number;
}

interface Series {
  readonly kind: string;
  readonly points: readonly { readonly value: number | null }[];
}

function isBytes(value: number | null | undefined): value is number {
  return value != null && Number.isFinite(value) && value >= 0;
}

export function memoryBytesOfSeries(series: readonly Series[]): number | null {
  const values = (series.find((one) => one.kind === KIND)?.points ?? []).map((point) => point.value).filter(isBytes);
  return values.at(-1) ?? null;
}

export function mergeMemoryBytes(known: number | null, samples: readonly Sample[]): number | null {
  const values = samples.filter((sample) => sample.kind === KIND && isBytes(sample.value)).map((sample) => sample.value);
  return values.at(-1) ?? known;
}

/** Rounded to the whole MiB, so that a percent rounded by the daemon does not move it from batch to batch. */
export function memoryLimit(used: number | null, percent: number | null): number | null {
  if (!isBytes(used) || percent === null || !Number.isFinite(percent) || percent <= 0) return null;
  return Math.max(1, Math.round((used * 100) / percent / MIB)) * MIB;
}

/** One decimal under 100, as `formatBytes` writes the data of the hub on the same page. */
function digits(value: number): string {
  return value >= 100 ? String(Math.round(value)) : value.toFixed(1);
}

export function usedOfText(used: number, limit: number): string {
  let step = 0;
  while (limit / 1024 ** step >= 1024 && step < IEC.length - 1) step += 1;
  const scale = 1024 ** step;
  return `${digits(used / scale)} of ${digits(limit / scale)} ${IEC[step]}`;
}

function cpuBar(percent: number | null): Bar {
  const tooltip = 'CPU: the share of the processors that the hub may use';
  // The note is visible where the bars have room for it (the Overview), so the meaning is not in a tooltip only.
  return { key: 'cpu', label: 'CPU', percent, text: percentText(percent), degraded: percent !== null && percent > DEGRADED_ABOVE, tooltip, note: 'of its processors' };
}

function memoryBar(percent: number | null, used: number | null): Bar {
  const limit = memoryLimit(used, percent);
  const degraded = percent !== null && percent > DEGRADED_ABOVE;
  if (used === null || limit === null) {
    return { key: 'memory', label: 'Memory', percent, text: percentText(percent), degraded, tooltip: 'Memory: the share of the limit of the hub', note: null };
  }
  const text = usedOfText(used, limit);
  const spoken = `${text}, ${percentText(percent)}${degraded ? ', degraded' : ''}`;
  return { key: 'memory', label: 'Memory', percent, text, degraded, tooltip: `Memory: ${text}, the limit of the hub`, note: null, spoken };
}

/** No disk bar: the disk of the hub has no total (HMD-072). */
export function containerRow(readings: BarReadings | null, source: MetricsSource | null | undefined, memoryBytes: number | null): BarRow {
  if (!hasMetrics(source)) return { kind: 'empty', text: 'No metrics' };
  if (readings === null) return { kind: 'empty', text: 'Waiting for metrics' };
  return { kind: 'bars', bars: [cpuBar(readings.cpuPercent), memoryBar(readings.memoryPercent, memoryBytes)] };
}
