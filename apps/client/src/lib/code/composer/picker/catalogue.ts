import type { CodeIconName } from '../../icons/icons';
import { CAN, type Can } from '../draft';
import { MAX_BARS, type EffortBars } from './glyph';
import { rankSearch } from './search';

/** A program that runs a chat. When it cannot be used, the words say why (03 A.2.3). */
export interface Provider {
  readonly id: string;
  readonly name: string;
  readonly icon: CodeIconName;
  readonly can: Can;
}

/** One level of reasoning effort of a model (03 A.3). */
export interface EffortLevel {
  readonly id: string;
  readonly label: string;
  /** Shown as a tip: the column is too narrow to print it. */
  readonly description?: string;
  readonly isDefault?: boolean;
  /** `off` is the zero of the scale (no bar is full); `named` is outside the scale and is always shown as its word. */
  readonly kind?: 'off' | 'named';
}

/** One speed of a model: the first of a model's speeds is the standard one (03 A.3.4). */
export interface Speed {
  readonly id: string;
  readonly label: string;
  readonly description?: string;
  /** The bolt drawn with it: one for fast, two for the fastest. */
  readonly bolt?: 'fast' | 'ultra';
}

/** A trait of a model that is on or off, such as thinking (03 A.3.3, "boolean option row"). */
export interface Toggle {
  readonly id: string;
  readonly label: string;
}

/** A model of a provider, as a row of the picker shows it (03 A.2.4). */
export interface Model {
  readonly id: string;
  /** The id of its provider. */
  readonly provider: string;
  readonly label: string;
  /** The quiet words after the name: who serves the model. */
  readonly subLabel?: string;
  /** It carries the "New" badge. */
  readonly isNew?: boolean;
  /** An older model: it stays under "Show more" unless it is a favourite. */
  readonly legacy?: boolean;
  readonly favourite: boolean;
  /** When it cannot be picked, the row is dimmed, carries "Unavailable" and tells the reason. */
  readonly can: Can;
  /** The heading of its effort levels; "Effort" when left out. */
  readonly effortLabel?: string;
  readonly efforts: readonly EffortLevel[];
  readonly speeds: readonly Speed[];
  readonly toggles: readonly Toggle[];
}

/** How much the agent may do without asking (03 A.4.1). */
export interface PermissionMode {
  readonly id: string;
  readonly label: string;
  readonly description: string;
  readonly icon: CodeIconName;
  /** When a policy forbids it, the reason replaces the description. */
  readonly can: Can;
}

/** Why the list has fewer models than the programs offer: they have not come yet, or they could not load. */
export interface ModelsNotice {
  /** The whole reason, as the runner gave it. */
  readonly words: string;
  /** The models could not load: the picker offers to ask again. */
  readonly failed: boolean;
}

/** All that the picker offers. The order of `providers` and of `models` is the order shown. */
export interface Catalogue {
  readonly providers: readonly Provider[];
  readonly models: readonly Model[];
  readonly modes: readonly PermissionMode[];
  /** Shown on a line of its own under the list, never in a row. */
  readonly notice?: ModelsNotice;
}

/** What is picked now. */
export interface Choice {
  readonly provider: string;
  readonly model: string;
  readonly effort: string | null;
  readonly speed: string | null;
  /** The ids of the toggles that are on. */
  readonly toggles: readonly string[];
  /** The id of the permission mode. */
  readonly mode: string;
}

/** The tab of the rail that lists the favourites of all providers; each other tab is a provider's id. */
export const FAVOURITES = 'favourites';

/** One row of the list: a model, or the row that shows or hides a provider's older models. */
export type Row =
  | { readonly kind: 'model'; readonly key: string; readonly model: Model; readonly provider: Provider }
  | { readonly kind: 'more'; readonly key: string; readonly provider: string; readonly expanded: boolean };

/** What the list shows: the open tab, the words in the search field, the providers whose older models are open. */
export interface ListView {
  readonly tab: string;
  readonly query: string;
  readonly expanded: readonly string[];
}

/** What the trigger shows after the model's name (03 A.3.1). */
export interface EffortSummary {
  /** The effort as a word; null when the bars say it, or when the model has no levels. */
  readonly word: string | null;
  readonly bars: EffortBars | null;
  readonly bolt: 'fast' | 'ultra' | null;
  /** All of it in words, for the accessible name: "High Fast · Thinking". Empty when there is nothing to say. */
  readonly detail: string;
}

/**
 * The four modes of 03 A.4.1, in their order. The source draws a lock for "Supervised" and a pen with a line for
 * "Auto-accept edits"; the icon list has neither, so the nearest are used.
 */
export const PERMISSION_MODES: readonly PermissionMode[] = [
  { id: 'approval-required', label: 'Supervised', description: 'Ask before commands and file changes.', icon: 'shield', can: CAN },
  { id: 'auto-accept-edits', label: 'Auto-accept edits', description: 'Auto-approve edits, ask before other actions.', icon: 'pencil', can: CAN },
  { id: 'auto', label: 'Auto', description: 'Supported providers approve routine actions; others still ask.', icon: 'shield-check', can: CAN },
  { id: 'full-access', label: 'Full access', description: 'Allow commands and edits without prompts.', icon: 'shield-off', can: CAN },
];

/** The mode a chat really runs in: the asked one when it is offered, else the first offered (03 A.4.1). */
export function settledMode(modes: readonly PermissionMode[], mode: string): PermissionMode | undefined {
  return modes.find((offered) => offered.id === mode) ?? modes[0];
}

/** The name of a model among all providers. */
export function modelKey(model: { readonly provider: string; readonly id: string }): string {
  return `${model.provider}:${model.id}`;
}

export function chosenModel(catalogue: Catalogue, value: Choice): Model | undefined {
  return catalogue.models.find((model) => model.provider === value.provider && model.id === value.model);
}

/** The tab the popup opens on: the favourites when one can be picked, unless the chosen model cannot be (03 A.2.3). */
export function openingTab(catalogue: Catalogue, value: Choice): string {
  if (chosenModel(catalogue, value)?.can.can === false) return value.provider;
  const usable = new Set(catalogue.providers.flatMap((provider) => (provider.can.can ? [provider.id] : [])));
  const pickable = catalogue.models.some((model) => model.favourite && model.can.can && usable.has(model.provider));
  return pickable ? FAVOURITES : value.provider;
}

/** The tab before or after `tab` among the favourites and the providers that can be used, round the ends (03 A.2.5). */
export function nextTab(catalogue: Catalogue, tab: string, direction: 1 | -1): string {
  const tabs = [FAVOURITES, ...catalogue.providers.flatMap((provider) => (provider.can.can ? [provider.id] : []))];
  const at = tabs.indexOf(tab);
  if (at < 0) return (direction === 1 ? tabs[0] : tabs.at(-1)) ?? FAVOURITES;
  return tabs[(at + direction + tabs.length) % tabs.length] ?? FAVOURITES;
}

function isOlder(model: Model): boolean {
  return model.legacy === true && !model.favourite;
}

/** A tab's models in the order shown: favourites first in a provider's tab; by provider in the favourites tab. */
function tabModels(catalogue: Catalogue, tab: string): Model[] {
  const order = new Map(catalogue.models.map((model, index) => [model, index]));
  const place = new Map(catalogue.providers.map((provider, index) => [provider.id, index]));
  const first = (model: Model): number => (tab === FAVOURITES ? (place.get(model.provider) ?? 0) : Number(!model.favourite));
  return catalogue.models
    .filter((model) => (tab === FAVOURITES ? model.favourite : model.provider === tab))
    .sort((a, b) => first(a) - first(b) || (order.get(a) ?? 0) - (order.get(b) ?? 0));
}

/**
 * The rows of the list (03 A.2.4). A query searches all providers, best match first. With no query a provider's tab
 * ends with the "Show more" row when it has older models, and shows them only when it is open.
 */
export function rowsOf(catalogue: Catalogue, view: ListView): Row[] {
  const providers = new Map(catalogue.providers.map((provider) => [provider.id, provider]));
  const row = (model: Model): Row[] => {
    const provider = providers.get(model.provider);
    return provider === undefined ? [] : [{ kind: 'model', key: modelKey(model), model, provider }];
  };
  if (view.query.trim() !== '') {
    const read = (model: Model) => ({ label: model.label, subLabel: model.subLabel, provider: providers.get(model.provider)?.name ?? '', favourite: model.favourite });
    return rankSearch(catalogue.models, view.query, read).flatMap(row);
  }
  const models = tabModels(catalogue, view.tab);
  if (view.tab === FAVOURITES || !models.some(isOlder)) return models.flatMap(row);
  const expanded = view.expanded.includes(view.tab);
  const shown = expanded ? [...models.filter((model) => !isOlder(model)), ...models.filter(isOlder)] : models.filter((model) => !isOlder(model));
  return [...shown.flatMap(row), { kind: 'more', key: `more:${view.tab}`, provider: view.tab, expanded }];
}

function canPick(row: Row): boolean {
  return row.kind === 'more' || row.model.can.can;
}

/** The keys of the first nine models that can be picked: the ones a jump key reaches (03 A.2.5). */
export function jumpKeys(rows: readonly Row[]): string[] {
  return rows.flatMap((row) => (row.kind === 'model' && row.model.can.can ? [row.key] : [])).slice(0, 9);
}

/**
 * The index to go to from `current` among `count` things after an arrow, Home or End; null when the key does not
 * move or there is nothing. `wrap` goes round the ends; without it the ends stop.
 */
export function nextIndex(key: string, current: number, count: number, wrap = false): number | null {
  if (count === 0) return null;
  if (key === 'Home') return 0;
  if (key === 'End') return count - 1;
  const step = key === 'ArrowDown' || key === 'ArrowRight' ? 1 : key === 'ArrowUp' || key === 'ArrowLeft' ? -1 : 0;
  if (step === 0) return null;
  if (current < 0) return step === 1 ? 0 : count - 1;
  const next = current + step;
  return wrap ? (next + count) % count : Math.max(0, Math.min(count - 1, next));
}

/** The key of the row to light after Up or Down from the row `current`, passing over the ones that cannot be picked. */
export function nextRow(key: string, current: string | null, rows: readonly Row[]): string | null {
  const open = rows.filter(canPick);
  const next = nextIndex(key, open.findIndex((row) => row.key === current), open.length, true);
  return next === null ? null : (open[next]?.key ?? null);
}

/** The row lit when the popup opens or the list changes: `wanted` when it is there to pick, else the first that is. */
export function litRow(rows: readonly Row[], wanted: string | null): string | null {
  const open = rows.filter(canPick);
  return (open.find((row) => row.key === wanted) ?? open[0])?.key ?? null;
}

function keep<T extends { readonly id: string }>(offered: readonly T[], id: string | null): T | undefined {
  return offered.find((one) => one.id === id);
}

/** The choice after `model` is picked: each setting stays when the model has it, else goes to the model's default. */
export function pickModel(value: Choice, model: Model): Choice {
  const effort = keep(model.efforts, value.effort) ?? model.efforts.find((level) => level.isDefault === true) ?? model.efforts[0];
  const speed = keep(model.speeds, value.speed) ?? model.speeds[0];
  return {
    ...value,
    provider: model.provider,
    model: model.id,
    effort: effort?.id ?? null,
    speed: speed?.id ?? null,
    toggles: value.toggles.filter((id) => model.toggles.some((toggle) => toggle.id === id)),
  };
}

/**
 * The bars for a level (03 A.3.2): one for each ordinary level, full up to the current one; an `off` level is no bar
 * full; a `named` level fills all. Null when the scale has under 2 or over 7 ordinary levels, or no level is current.
 */
export function effortBars(efforts: readonly EffortLevel[], current: string | null): EffortBars | null {
  const level = keep(efforts, current);
  const scale = efforts.filter((one) => one.kind !== 'named');
  if (level === undefined || scale.length < 2 || scale.length > MAX_BARS) return null;
  const fromOff = scale[0]?.kind === 'off';
  const count = fromOff ? scale.length - 1 : scale.length;
  if (level.kind === 'named') return { count, filled: count };
  const index = scale.indexOf(level);
  return { count, filled: fromOff ? index : index + 1 };
}

/**
 * What the trigger says of the choice after the model's name (03 A.3.1): the effort as bars, or as its word when the
 * reader asked for words, the level is a named one or the scale cannot be drawn; then the bolt of a fast speed.
 */
export function describeEffort(model: Model | undefined, value: Choice, display: 'label' | 'bars'): EffortSummary {
  if (model === undefined) return { word: null, bars: null, bolt: null, detail: '' };
  const level = keep(model.efforts, value.effort);
  const bars = effortBars(model.efforts, value.effort);
  const worded = display === 'label' || bars === null || level?.kind === 'named';
  const speed = keep(model.speeds, value.speed);
  const bolt = speed?.bolt ?? null;
  const reasoning = [level?.label, bolt === null ? undefined : speed?.label].filter((part) => part !== undefined).join(' ');
  const on = model.toggles.filter((toggle) => value.toggles.includes(toggle.id)).map((toggle) => toggle.label);
  return {
    word: worded ? (level?.label ?? null) : null,
    bars: worded ? null : bars,
    bolt,
    detail: [reasoning, ...on].filter((part) => part !== '').join(' · '),
  };
}

/** The families a maker puts before the version of a model; T3 Code's `modelShortName` drops them. */
const FAMILIES = /^(claude|gpt|gemini|grok)[\s-]+/i;

/**
 * The short name of a model for the trigger and the head of the levels (T3 Code `modelShortName.ts`): a maker's
 * "Vendor: " and its family go when what is left names a model, that is, holds a version number. "GPT-5.4-Codex" is
 * "5.4 Codex"; "Claude Code default" stays whole. Pure.
 */
export function shortName(label: string): string {
  const bare = label.replace(/^[^:]{1,24}:\s+/, '');
  const rest = bare.replace(FAMILIES, '');
  return rest !== bare && /\d/.test(rest) ? rest.replace(/-/g, ' ') : bare;
}
