import { latestOf, type ChartPoint } from './plot';

interface LabelledSeries {
  readonly label: string;
  readonly points: readonly ChartPoint[];
}

interface ChartText {
  readonly title: string;
  readonly series: readonly LabelledSeries[];
  readonly top: number;
  readonly format: (value: number) => string;
  readonly threshold?: number;
  readonly empty?: string;
}

/** With more than one series each latest value is named by its series, so the text never relies on colour. */
export function chartLabel({ title, series, top, format, threshold, empty }: ChartText): string {
  const latest = series.map((one) => ({ label: one.label, value: latestOf(one.points) }));
  if (latest.every((one) => one.value === null)) return empty ?? `${title}: no values yet`;
  const said = (value: number | null) => (value === null ? 'no value' : format(value));
  const values =
    latest.length === 1
      ? `latest ${said(latest[0]?.value ?? null)}`
      : latest.map((one) => `${one.label} latest ${said(one.value)}`).join(', ');
  const marked = threshold === undefined ? '' : `, threshold line at ${format(threshold)}`;
  return `${title}, ${values}, top of the chart ${format(top)}${marked}`;
}
