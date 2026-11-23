import { describe, expect, it } from 'vitest';
import { cannot } from '../draft';
import { FAVOURITES, PERMISSION_MODES, describeEffort, effortBars, jumpKeys, litRow, nextIndex, nextRow, nextTab, openingTab, pickModel, rowsOf, settledMode, shortName } from './catalogue';
import type { Catalogue, EffortLevel, Model } from './catalogue';
import { pickerCatalogue, pickerChoice } from './fixture';
import { barHeights, glyphBox } from './glyph';
import { jumpLabel, popupIntent, searchIntent, type KeyPress } from './keys';
import { scoreSearch } from './search';

const labels = (catalogue: Catalogue, tab: string, query = '', expanded: string[] = []): string[] =>
  rowsOf(catalogue, { tab, query, expanded }).map((row) => (row.kind === 'model' ? row.model.label : row.expanded ? 'Show less' : 'Show more'));

function model(id: string): Model {
  const found = pickerCatalogue.models.find((one) => one.id === id);
  if (found === undefined) throw new Error(`no model ${id}`);
  return found;
}

const press = (key: string, held: Partial<KeyPress> = {}): KeyPress => ({ key, shiftKey: false, altKey: false, ctrlKey: false, metaKey: false, ...held });

describe('the rows of the list (03 A.2.4)', () => {
  it('lists a provider with its favourites first and its older models under "Show more"', () => {
    expect(labels(pickerCatalogue, 'claude-code')).toEqual(['Opus 5', 'Sonnet 5', 'Haiku 5', 'Show more']);
    expect(labels(pickerCatalogue, 'claude-code', '', ['claude-code'])).toEqual(['Opus 5', 'Sonnet 5', 'Haiku 5', 'Opus 4', 'Sonnet 4', 'Show less']);
    expect(labels(pickerCatalogue, 'codex')).toEqual(['GPT-6 Codex', 'GPT-6 Codex Mini', 'GPT-6 Codex Preview']);
  });

  it('lists the favourites of all providers in the order of the providers', () => {
    const codexFirst = { ...pickerCatalogue, providers: [...pickerCatalogue.providers].reverse() };
    expect(labels(pickerCatalogue, FAVOURITES)).toEqual(['Opus 5', 'Sonnet 5', 'GPT-6 Codex']);
    expect(labels(codexFirst, FAVOURITES)).toEqual(['GPT-6 Codex', 'Opus 5', 'Sonnet 5']);
  });

  it('searches all providers whatever the tab, older models too, best match first', () => {
    expect(labels(pickerCatalogue, 'codex', 'opus')).toEqual(['Opus 5', 'Opus 4']);
    expect(labels(pickerCatalogue, 'claude-code', 'codex mini')).toEqual(['GPT-6 Codex Mini']);
    expect(labels(pickerCatalogue, FAVOURITES, 'sonnet 4')).toEqual(['Sonnet 4']);
    expect(labels(pickerCatalogue, FAVOURITES, 'claude')).toHaveLength(5);
    expect(labels(pickerCatalogue, FAVOURITES, 'zzz')).toEqual([]);
  });

  it('ranks a whole name over a start, a start over a part, and letters apart last', () => {
    const named = (label: string) => ({ label, provider: 'Codex', favourite: false });
    const order = ['mini', 'mini max', 'gpt-mini', 'gemini', 'minimal i'].map((label) => scoreSearch(named(label), 'mini'));
    expect(order).toEqual([-1000, 2, 4, 6, 2]);
    expect(scoreSearch(named('Sonnet'), 'snt')).toBeGreaterThan(100);
    expect(scoreSearch(named('Sonnet'), 'st')).toBeNull();
    expect(scoreSearch({ ...named('mini max'), favourite: true }, 'mini')).toBe(-22);
  });
});

describe('keys of the picker (03 A.2.5)', () => {
  const rows = rowsOf(pickerCatalogue, { tab: 'codex', query: '', expanded: [] });

  it('moves the lit row past a model that cannot be picked and round the ends', () => {
    expect(nextRow('ArrowDown', 'codex:gpt-6-codex', rows)).toBe('codex:gpt-6-codex-mini');
    expect(nextRow('ArrowDown', 'codex:gpt-6-codex-mini', rows)).toBe('codex:gpt-6-codex');
    expect(nextRow('ArrowUp', 'codex:gpt-6-codex', rows)).toBe('codex:gpt-6-codex-mini');
    expect(nextRow('ArrowDown', null, [])).toBeNull();
  });

  it('lights the wanted row when it can be picked, else the first that can', () => {
    expect(litRow(rows, 'codex:gpt-6-codex-mini')).toBe('codex:gpt-6-codex-mini');
    expect(litRow(rows, 'codex:gpt-6-codex-preview')).toBe('codex:gpt-6-codex');
    expect(litRow(rows, 'claude-code:opus-5')).toBe('codex:gpt-6-codex');
    expect(litRow([], 'claude-code:opus-5')).toBeNull();
  });

  it('gives a jump key to the first nine models that can be picked', () => {
    expect(jumpKeys(rows)).toEqual(['codex:gpt-6-codex', 'codex:gpt-6-codex-mini']);
    expect(jumpKeys(rowsOf(pickerCatalogue, { tab: 'claude-code', query: '', expanded: [] }))).toHaveLength(3);
    expect([jumpLabel(0, true), jumpLabel(8, false)]).toEqual(['⌘1', 'Ctrl+9']);
  });

  it('goes round the favourites and the providers that can be used', () => {
    const oneOff: Catalogue = { ...pickerCatalogue, providers: pickerCatalogue.providers.map((provider) => (provider.id === 'codex' ? { ...provider, can: cannot('Codex: Not ready.') } : provider)) };
    expect(nextTab(pickerCatalogue, FAVOURITES, 1)).toBe('claude-code');
    expect(nextTab(pickerCatalogue, 'codex', 1)).toBe(FAVOURITES);
    expect(nextTab(pickerCatalogue, FAVOURITES, -1)).toBe('codex');
    expect(nextTab(oneOff, 'claude-code', 1)).toBe(FAVOURITES);
  });

  it('stops at the ends of a column and goes round a rail', () => {
    expect([nextIndex('ArrowDown', 2, 3), nextIndex('ArrowUp', 0, 3), nextIndex('ArrowRight', 0, 3)]).toEqual([2, 0, 1]);
    expect([nextIndex('ArrowDown', 2, 3, true), nextIndex('ArrowUp', 0, 3, true)]).toEqual([0, 2]);
    expect([nextIndex('Home', 2, 3), nextIndex('End', 0, 3), nextIndex('a', 0, 3), nextIndex('ArrowDown', 0, 0)]).toEqual([0, 2, null, null]);
  });

  it('reads the keys of the search field', () => {
    const at = { query: 'op', caret: 2, rail: false };
    expect(searchIntent(press('ArrowDown'), at)).toEqual({ to: 'move', key: 'ArrowDown' });
    expect(searchIntent(press('Enter'), at)).toEqual({ to: 'activate' });
    expect(searchIntent(press('ArrowRight'), at)).toEqual({ to: 'effort' });
    expect(searchIntent(press('ArrowRight'), { ...at, caret: 1 })).toEqual({ to: 'type' });
    expect(searchIntent(press('ArrowLeft'), at)).toEqual({ to: 'type' });
    expect(searchIntent(press('ArrowLeft'), { query: '', caret: 0, rail: true })).toEqual({ to: 'rail' });
    expect(searchIntent(press('Tab', { shiftKey: true }), { ...at, rail: true })).toEqual({ to: 'rail' });
    expect(searchIntent(press('Tab', { shiftKey: true }), at)).toEqual({ to: 'type' });
    expect(searchIntent(press('ArrowDown', { metaKey: true, shiftKey: true }), at)).toEqual({ to: 'type' });
  });

  it('reads the keys of the whole popup', () => {
    expect(popupIntent(press('Escape'))).toEqual({ to: 'close' });
    expect(popupIntent(press('ArrowDown', { metaKey: true, shiftKey: true }))).toEqual({ to: 'tab', direction: 1 });
    expect(popupIntent(press('ArrowUp', { ctrlKey: true, shiftKey: true }))).toEqual({ to: 'tab', direction: -1 });
    expect(popupIntent(press('3', { metaKey: true }))).toEqual({ to: 'jump', index: 2 });
    expect(popupIntent(press('3'))).toEqual({ to: 'pass' });
    expect(popupIntent(press('0', { ctrlKey: true }))).toEqual({ to: 'pass' });
    expect(popupIntent(press('3', { metaKey: true, altKey: true }))).toEqual({ to: 'pass' });
  });
});

describe('the tab the popup opens on (03 A.2.3)', () => {
  it('opens on the favourites when one can be picked, else on the provider in use', () => {
    const none: Catalogue = { ...pickerCatalogue, models: pickerCatalogue.models.map((one) => ({ ...one, favourite: false })) };
    expect(openingTab(pickerCatalogue, pickerChoice)).toBe(FAVOURITES);
    expect(openingTab(none, pickerChoice)).toBe('claude-code');
    expect(openingTab(pickerCatalogue, { ...pickerChoice, provider: 'codex', model: 'gpt-6-codex-preview' })).toBe('codex');
  });
});

describe('a pick of a model', () => {
  it('keeps each setting the new model has and takes its default for the rest', () => {
    expect(pickModel(pickerChoice, model('gpt-6-codex'))).toEqual({ provider: 'codex', model: 'gpt-6-codex', effort: 'high', speed: 'standard', toggles: [], mode: 'approval-required' });
    expect(pickModel({ ...pickerChoice, effort: 'max', speed: 'fast' }, model('gpt-6-codex-mini'))).toMatchObject({ effort: 'medium', speed: null });
    expect(pickModel(pickerChoice, model('haiku-5'))).toMatchObject({ effort: null, speed: null, toggles: [] });
  });
});

describe('the effort after the name of the model (03 A.3.1, A.3.2)', () => {
  const claude = model('opus-5');
  const codex = model('gpt-6-codex');
  const levels = (count: number): EffortLevel[] => Array.from({ length: count }, (_, index) => ({ id: String(index), label: String(index) }));

  it('draws one bar for each ordinary level, full up to the level in use', () => {
    expect(effortBars(claude.efforts, 'high')).toEqual({ count: 4, filled: 3 });
    expect(effortBars(claude.efforts, 'max')).toEqual({ count: 4, filled: 4 });
    expect(effortBars(codex.efforts, 'none')).toEqual({ count: 4, filled: 0 });
    expect(effortBars(codex.efforts, 'xhigh')).toEqual({ count: 4, filled: 4 });
    expect([effortBars(claude.efforts, null), effortBars(levels(1), '0'), effortBars(levels(8), '0')]).toEqual([null, null, null]);
    expect(effortBars(levels(7), '6')).toEqual({ count: 7, filled: 7 });
  });

  it('says the effort as a word or as bars, then the bolt of a fast speed', () => {
    expect(describeEffort(claude, pickerChoice, 'label')).toEqual({ word: 'High', bars: null, bolt: null, detail: 'High · Thinking' });
    expect(describeEffort(claude, pickerChoice, 'bars')).toMatchObject({ word: null, bars: { count: 4, filled: 3 } });
    expect(describeEffort(claude, { ...pickerChoice, effort: 'max', speed: 'fast', toggles: [] }, 'bars')).toEqual({ word: 'Max', bars: null, bolt: 'fast', detail: 'Max Fast' });
    expect(describeEffort(codex, { ...pickerChoice, effort: 'low', speed: 'ultrafast' }, 'bars')).toMatchObject({ bars: { count: 4, filled: 1 }, bolt: 'ultra', detail: 'Low Ultrafast' });
    expect(describeEffort(model('haiku-5'), pickerChoice, 'label')).toEqual({ word: null, bars: null, bolt: null, detail: '' });
    expect(describeEffort(undefined, pickerChoice, 'label')).toEqual({ word: null, bars: null, bolt: null, detail: '' });
  });

  it('gives the bars the heights of the source, in whole pixels on one base line', () => {
    const heights = (maxBar: number): number[][] => [2, 3, 4, 5, 6, 7].map((count) => barHeights(count, maxBar));
    expect(heights(12)).toEqual([[6, 12], [6, 9, 12], [6, 8, 10, 12], [4, 6, 8, 10, 12], [7, 8, 9, 10, 11, 12], [6, 7, 8, 9, 10, 11, 12]]);
    expect(heights(10)).toEqual([[5, 10], [4, 7, 10], [4, 6, 8, 10], [6, 7, 8, 9, 10], [5, 6, 7, 8, 9, 10], [4, 5, 6, 7, 8, 9, 10]]);
    const box = glyphBox({ count: 4, filled: 3 }, 'sm');
    expect([box.width, box.height, box.barWidth]).toEqual([14, 14, 2]);
    expect(box.bars).toEqual([
      { x: 0, y: 6, height: 6, full: true },
      { x: 4, y: 4, height: 8, full: true },
      { x: 8, y: 2, height: 10, full: true },
      { x: 12, y: 0, height: 12, full: false },
    ]);
    expect(glyphBox({ count: 2, filled: 0 }, 'xs')).toMatchObject({ width: 6, height: 12 });
  });
});

describe('the permission modes (03 A.4.1)', () => {
  it('has the four modes in their order, each with its words', () => {
    expect(PERMISSION_MODES.map((mode) => `${mode.id}/${mode.label}`)).toEqual(['approval-required/Supervised', 'auto-accept-edits/Auto-accept edits', 'auto/Auto', 'full-access/Full access']);
  });

  it('falls back to the first offered mode when the asked one is not offered', () => {
    const offered = PERMISSION_MODES.slice(0, 2);
    expect(settledMode(offered, 'auto-accept-edits')?.id).toBe('auto-accept-edits');
    expect(settledMode(offered, 'full-access')?.id).toBe('approval-required');
    expect(settledMode([], 'auto')).toBeUndefined();
  });
});

describe("T3 Code's short name of a model", () => {
  it('drops the family before a version, and keeps a name with no version whole', () => {
    expect(['Claude Opus 4.5', 'GPT-5.4-Codex', 'OpenAI: GPT-5.4 mini', 'Opus 4.7', 'Claude Code default', 'Codex default'].map(shortName)).toEqual([
      'Opus 4.5',
      '5.4 Codex',
      '5.4 mini',
      'Opus 4.7',
      'Claude Code default',
      'Codex default',
    ]);
  });
});
