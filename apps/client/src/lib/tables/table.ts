/** What a severity cell shows: a word and a dot in the severity's colour (UI-003). */
export type Severity = 'info' | 'warning' | 'error';

export const SEVERITY_WORDS: Record<Severity, string> = { info: 'Info', warning: 'Warning', error: 'Error' };

/** How a column is drawn. `number` is right-aligned in tabular figures (UI-025); `custom` is drawn by the table's `cell` snippet. */
export type ColumnKind = 'text' | 'number' | 'severity' | 'custom';

export interface Column<Row> {
  readonly key: string;
  readonly header: string;
  readonly kind?: ColumnKind;
  readonly value?: (row: Row) => string;
  readonly severity?: (row: Row) => Severity;
  /** In rem; the table never goes narrower than the sum of these. */
  readonly min: number;
  /** In rem. */
  readonly max?: number;
  readonly flex?: boolean;
}

export function columnWidth<Row>(column: Column<Row>): string | undefined {
  if (column.flex === true) return undefined;
  return `${String(column.max ?? column.min)}rem`;
}

export function tableMinWidth<Row>(columns: readonly Column<Row>[]): string {
  return `${String(columns.reduce((sum, column) => sum + column.min, 0))}rem`;
}

export type BodyEntry<Row> = { readonly group: string } | { readonly row: Row };

export function withGroupHeaders<Row>(rows: readonly Row[], groupOf?: (row: Row) => string): readonly BodyEntry<Row>[] {
  if (groupOf === undefined) return rows.map((row) => ({ row }));
  const entries: BodyEntry<Row>[] = [];
  let current: string | undefined;
  for (const row of rows) {
    const group = groupOf(row);
    if (group !== current) entries.push({ group });
    current = group;
    entries.push({ row });
  }
  return entries;
}

function startOfDay(moment: Date): number {
  return new Date(moment.getFullYear(), moment.getMonth(), moment.getDate()).getTime();
}

const DAY_MS = 86_400_000;
const DATE = new Intl.DateTimeFormat('en-GB', { day: 'numeric', month: 'short', year: 'numeric' });

/** Calendar days in the local time zone. */
export function dayLabel(at: Date, now: Date): string {
  const days = Math.round((startOfDay(now) - startOfDay(at)) / DAY_MS);
  if (days === 0) return 'Today';
  return days === 1 ? 'Yesterday' : DATE.format(at);
}
