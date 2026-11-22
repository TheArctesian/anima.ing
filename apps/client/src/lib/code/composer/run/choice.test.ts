import { describe, expect, it } from 'vitest';
import type { RunRow } from '$lib/agent-threads/live/run';
import { catalogueOf, changeOf, type EffortOfRow } from './choice';

const row: RunRow & EffortOfRow = { id: 'r1', program: 'codex', folder: '/work/anima', mode: 'ask', model: 'gpt', state: 'idle', session: null, title: 'Fix the gate', started: new Date(0), ended: null, open: [], effort: 'medium' };
const usable = (of: RunRow & EffortOfRow, changes: boolean): string[] => catalogueOf(of, changes).catalogue.modes.filter((mode) => mode.can.can).map((mode) => mode.id);

describe('the choice of a chat that the runner drives', () => {
  it('shows the model, the mode and the effort of the row', () => {
    const { catalogue, choice } = catalogueOf(row, true);
    expect(choice).toMatchObject({ provider: 'codex', model: 'gpt', mode: 'approval-required', effort: 'medium' });
    expect(catalogue.models).toHaveLength(1);
    expect(catalogueOf({ ...row, model: null, effort: null }, true).choice).toMatchObject({ model: 'default', effort: null });
  });

  it('offers each effort to Codex, and to Claude Code only the one it runs with', () => {
    expect(catalogueOf(row, true).catalogue.models[0]?.efforts.map((level) => level.label)).toEqual(['Low', 'Medium', 'High', 'Extra high', 'Max']);
    expect(catalogueOf({ ...row, program: 'claude_code' }, true).catalogue.models[0]?.efforts.map((level) => level.id)).toEqual(['medium']);
    expect(catalogueOf(row, false).catalogue.models[0]?.efforts.map((level) => level.id)).toEqual(['medium']);
    expect(catalogueOf({ ...row, effort: null }, false).catalogue.models[0]?.efforts).toEqual([]);
  });

  it('says why for a mode the runner or the program lacks, where the mode can be changed', () => {
    expect(usable(row, true)).toEqual(['approval-required', 'auto-accept-edits', 'full-access']);
    expect(usable({ ...row, program: 'claude_code' }, true)).toEqual(['approval-required', 'auto-accept-edits', 'full-access', 'plan']);
    expect(catalogueOf(row, true).catalogue.modes.find((mode) => mode.id === 'plan')?.can).toEqual({ can: false, reason: 'Codex has no plan mode.' });
    expect(usable(row, false)).toHaveLength(5);
  });

  it('asks the runner for what the new choice changes, and for nothing when it changes nothing', () => {
    const { choice } = catalogueOf(row, true);
    expect(changeOf(row, choice)).toBeNull();
    expect(changeOf(row, { ...choice, mode: 'full-access' })).toEqual({ mode: 'full_access' });
    expect(changeOf(row, { ...choice, effort: 'high' })).toEqual({ effort: 'high' });
    expect(changeOf(row, { ...choice, mode: 'plan', effort: 'max' })).toEqual({ mode: 'plan', effort: 'max' });
    // A mode the runner does not have, a model it cannot change, an effort that is no level: nothing is asked.
    expect(changeOf(row, { ...choice, mode: 'auto', model: 'other', effort: 'none' })).toBeNull();
  });
});
