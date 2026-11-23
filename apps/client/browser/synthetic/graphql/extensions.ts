// Only the fixed extension is on unless a screen sets more on, so the bar of the other screens stays the same (EXT-001).

interface ExtensionRow {
  readonly key: string;
  readonly name: string;
  readonly built: boolean;
  readonly fixed: boolean;
}

const ROWS: readonly ExtensionRow[] = [
  { key: 'projects', name: 'Projects', built: true, fixed: false },
  { key: 'integrations', name: 'Integrations', built: true, fixed: true },
  { key: 'secrets', name: 'Secrets', built: false, fixed: false },
  { key: 'model_gateway', name: 'Model gateway', built: false, fixed: false },
  { key: 'map', name: 'Map', built: false, fixed: false },
  { key: 'writing', name: 'Writing', built: false, fixed: false },
  { key: 'people', name: 'People', built: false, fixed: false },
  { key: 'money', name: 'Money', built: false, fixed: false },
  { key: 'history', name: 'History', built: false, fixed: false },
];

export function extensionRows(on: readonly string[] = []): readonly Record<string, unknown>[] {
  return ROWS.map((row) => ({ ...row, dataClass: 'A', on: row.fixed || on.includes(row.key), needs: [], worksWith: [], neededBy: [], navigation: null, changedAt: null }));
}
