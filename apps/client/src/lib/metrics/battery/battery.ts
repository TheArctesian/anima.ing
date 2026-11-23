import type { BatteryState } from '$lib/hub/generated/graphql';
import { percentText, type Bar } from '../bars/bars';

/** Structural, so the `batteries` of any device query fits (MET-032). */
export interface BatteryListing {
  readonly name: string;
  readonly state: BatteryState;
}

/** The latest charge of each battery, 0 to 100, by battery name. */
export type Charges = ReadonlyMap<string, number>;

export const NO_CHARGES: Charges = new Map();

export interface BatteryBar extends Bar {
  readonly key: 'battery';
  readonly charging: boolean;
  readonly low: boolean;
}

/** Warning thresholds while the battery discharges (MET-032). */
export const LOW_AT = 20;
export const VERY_LOW_AT = 10;

const KIND = 'BATTERY_PERCENT';

const STATE_WORDS: Record<BatteryState, string> = {
  CHARGING: 'charging',
  DISCHARGING: 'on battery',
  FULL: 'full',
  NOT_CHARGING: 'plugged in, not charging',
  UNKNOWN: 'state unknown',
};

interface Sample {
  readonly kind: string;
  readonly label: string;
  readonly value: number;
}

interface Series {
  readonly kind: string;
  readonly label: string;
  readonly points: readonly { readonly value: number }[];
}

function isCharge(value: number): boolean {
  return Number.isFinite(value) && value >= 0 && value <= 100;
}

export function mergeCharges(known: Charges, samples: readonly Sample[]): Charges {
  const charges = samples.filter((sample) => sample.kind === KIND && isCharge(sample.value));
  if (charges.length === 0) return known;
  return new Map([...known, ...charges.map((sample) => [sample.label, sample.value] as const)]);
}

export function chargesOfSeries(series: readonly Series[]): Charges {
  return new Map(
    series.flatMap((one) => {
      const last = one.points.at(-1);
      return one.kind === KIND && last !== undefined && isCharge(last.value) ? [[one.label, last.value] as const] : [];
    }),
  );
}

interface Reading {
  readonly name: string;
  readonly state: BatteryState;
  readonly percent: number | null;
}

function warningOf(reading: Reading): string {
  if (reading.state !== 'DISCHARGING' || reading.percent === null || reading.percent > LOW_AT) return '';
  return reading.percent <= VERY_LOW_AT ? 'very low' : 'low';
}

function spokenOf(reading: Reading): string {
  return [percentText(reading.percent), warningOf(reading), STATE_WORDS[reading.state]].filter((part) => part !== '').join(', ');
}

function byCharge(a: Reading, b: Reading): number {
  return (a.percent ?? Number.POSITIVE_INFINITY) - (b.percent ?? Number.POSITIVE_INFINITY);
}

/** With several batteries the meter shows the lowest charge, the one that ends first (MET-032). */
export function batteryBar(batteries: readonly BatteryListing[], charges: Charges | undefined): BatteryBar | null {
  const readings = batteries.map(({ name, state }) => ({ name, state, percent: charges?.get(name) ?? null })).sort(byCharge);
  const shown = readings[0];
  if (shown === undefined) return null;
  const warning = warningOf(shown);
  const several = readings.length > 1;
  const spoken = several ? readings.map((one) => `${one.name} ${spokenOf(one)}`).join('; ') : spokenOf(shown);
  return {
    key: 'battery',
    label: 'Battery',
    percent: shown.percent,
    text: percentText(shown.percent),
    degraded: warning !== '',
    tooltip: `Battery: ${spoken}`,
    note: warning !== '' ? warning : shown.state === 'CHARGING' ? 'charging' : several ? `lowest of ${String(readings.length)}` : null,
    spoken,
    charging: shown.state === 'CHARGING',
    low: warning !== '',
  };
}
