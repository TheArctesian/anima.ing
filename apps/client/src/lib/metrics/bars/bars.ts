import type { MetricsSource } from '$lib/hub/generated/graphql';
import { LEGACY_IOS_DISK_MOUNT } from '../storage/storage';
import { DEGRADED_ABOVE } from '../units';
import { hasMetrics } from '../source/has-metrics';
import type { BatteryBar } from '../battery/battery';

/** The readings the bars draw. Structural, so a hub `CardReadings` fragment fits without this folder knowing devices. */
export interface BarReadings {
  readonly at?: string;
  readonly cpuPercent: number | null;
  readonly memoryPercent: number | null;
  readonly diskPercent: number | null;
  /** The fullest disk's mount (MET-030). */
  readonly diskMount: string | null;
  /** Unmounted drives are counted. */
  readonly otherDrives: number;
  readonly vramPercent: number | null;
  readonly vramGpu: { readonly index: number; readonly name: string; readonly hasVram: boolean } | null;
}

export interface Bar {
  readonly key: 'cpu' | 'memory' | 'disk' | 'vram' | 'battery';
  readonly label: string;
  readonly percent: number | null;
  readonly text: string;
  readonly degraded: boolean;
  readonly tooltip: string;
  /** MET-030. */
  readonly note: string | null;
  readonly spoken?: string;
}

export type BarRow =
  | { readonly kind: 'bars'; readonly bars: readonly Bar[] }
  | { readonly kind: 'empty'; readonly text: string };

const UNKNOWN = '—';

/** A missing value is a dash, never 0 (MET-004, MET-007). */
export function percentText(percent: number | null): string {
  return percent === null ? UNKNOWN : `${String(Math.round(percent))} %`;
}

function bar(key: Bar['key'], label: string, percent: number | null, tooltip = label): Bar {
  return { key, label, percent, text: percentText(percent), degraded: percent !== null && percent > DEGRADED_ABOVE, tooltip, note: null };
}

export function otherDrivesText(count: number): string | null {
  if (!Number.isInteger(count) || count < 1) return null;
  return `+${String(count)} ${count === 1 ? 'drive' : 'drives'}`;
}

function diskBar(readings: BarReadings): Bar {
  const label = readings.diskMount === null ? 'Disk' : `Disk ${readings.diskMount}`;
  const note = otherDrivesText(readings.otherDrives);
  const tooltip = note === null ? label : `${label}, ${note.slice(1)} more on this device`;
  return { ...bar('disk', label, readings.diskPercent, tooltip), note };
}

/** VRAM only for a GPU with its own memory (MET-007, MET-008). */
function barsOf(readings: BarReadings): readonly Bar[] {
  const bars = [
    bar('cpu', 'CPU', readings.cpuPercent),
    bar('memory', 'Memory', readings.memoryPercent),
  ];
  // Older iOS reporters send raw filesystem blocks under this label.
  if (readings.diskMount !== LEGACY_IOS_DISK_MOUNT) bars.push(diskBar(readings));
  const gpu = readings.vramGpu;
  if (gpu?.hasVram === true && readings.vramPercent !== null) {
    bars.push(bar('vram', 'VRAM', readings.vramPercent, `VRAM, GPU ${String(gpu.index)}: ${gpu.name}`));
  }
  return bars;
}

/** The battery comes last (6.1, MET-007, MET-032). */
export function barRow(readings: BarReadings | null, source: MetricsSource | null | undefined, battery: BatteryBar | null = null): BarRow {
  if (!hasMetrics(source)) return { kind: 'empty', text: 'No metrics' };
  if (readings === null) return { kind: 'empty', text: 'Waiting for metrics' };
  const bars = barsOf(readings);
  return { kind: 'bars', bars: battery === null ? bars : [...bars, battery] };
}
