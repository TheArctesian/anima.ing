import type { Chart } from './overview-charts';

/** Series per chart of each device's last answer, so its loading skeleton matches (MOT-007). */
const last = new Map<string, readonly number[]>();

/** The shape outlives the page, so a revisit draws the right skeleton. */
export function rememberChartShape(deviceId: string, charts: readonly Pick<Chart, 'series'>[]): void {
  last.set(
    deviceId,
    charts.map((chart) => chart.series.length),
  );
}

export function chartShapeOf(deviceId: string): readonly number[] | null {
  return last.get(deviceId) ?? null;
}

export function forgetChartShapes(): void {
  last.clear();
}
