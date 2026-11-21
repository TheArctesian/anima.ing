/**
 * Selection in the inbox (02 section 5.11): a click opens the thread; ⌘-click (Ctrl-click off macOS) toggles a row in
 * a multi-selection; Shift-click selects the range from the last row clicked. Pure.
 */

export interface Selection {
  readonly ids: ReadonlySet<string>;
  /** The row a range starts from: the last one clicked or toggled. */
  readonly anchor: string | null;
}

export const NO_SELECTION: Selection = { ids: new Set(), anchor: null };

export interface Click {
  /** ⌘ on macOS, Ctrl elsewhere. */
  readonly toggle: boolean;
  readonly range: boolean;
}

/** What a click on row `id` does: the new selection, and whether the thread opens. `order` is the list as shown. */
export function clicked(selection: Selection, id: string, order: readonly string[], click: Click): { readonly selection: Selection; readonly open: boolean } {
  if (click.range) {
    const from = order.indexOf(selection.anchor ?? id);
    const to = order.indexOf(id);
    if (from === -1 || to === -1) return { selection: { ids: new Set([id]), anchor: id }, open: false };
    const [low, high] = from <= to ? [from, to] : [to, from];
    return { selection: { ids: new Set(order.slice(low, high + 1)), anchor: selection.anchor ?? id }, open: false };
  }
  if (click.toggle) {
    const ids = new Set(selection.ids);
    if (ids.has(id)) ids.delete(id);
    else ids.add(id);
    return { selection: { ids, anchor: id }, open: false };
  }
  return { selection: { ids: new Set(), anchor: id }, open: true };
}

/** The selection without the rows that left the list. */
export function kept(selection: Selection, order: readonly string[]): Selection {
  const shown = new Set(order);
  const ids = new Set([...selection.ids].filter((id) => shown.has(id)));
  if (ids.size === selection.ids.size) return selection;
  return { ids, anchor: selection.anchor !== null && shown.has(selection.anchor) ? selection.anchor : null };
}

/** Whether a pointer event toggles on this platform: ⌘ on macOS, Ctrl elsewhere. */
export function togglesOn(event: Pick<MouseEvent, 'metaKey' | 'ctrlKey'>, mac: boolean): boolean {
  return mac ? event.metaKey : event.ctrlKey;
}
