import { CAN, cannot } from '../draft';
import { PERMISSION_MODES, type Catalogue, type Choice, type EffortLevel, type Model, type Speed } from './catalogue';

/* A made-up catalogue for the specimen page and the tests: the two programs Anima drives, with invented models. */

const claudeEfforts: readonly EffortLevel[] = [
  { id: 'low', label: 'Low' },
  { id: 'medium', label: 'Medium' },
  { id: 'high', label: 'High', isDefault: true },
  { id: 'xhigh', label: 'Extra High', description: 'Thinks longer on hard problems; slower and costs more.' },
  { id: 'max', label: 'Max', kind: 'named', description: 'No limit on thinking.' },
];

const codexEfforts: readonly EffortLevel[] = [
  { id: 'none', label: 'None', kind: 'off' },
  { id: 'low', label: 'Low' },
  { id: 'medium', label: 'Medium', isDefault: true },
  { id: 'high', label: 'High' },
  { id: 'xhigh', label: 'Extra High' },
];

const twoSpeeds: readonly Speed[] = [
  { id: 'standard', label: 'Standard' },
  { id: 'fast', label: 'Fast', bolt: 'fast' },
];

const threeSpeeds: readonly Speed[] = [...twoSpeeds, { id: 'ultrafast', label: 'Ultrafast', bolt: 'ultra', description: 'Even faster, more expensive' }];

const thinking = [{ id: 'thinking', label: 'Thinking' }] as const;

function claude(model: Pick<Model, 'id' | 'label'> & Partial<Model>): Model {
  return { provider: 'claude-code', favourite: false, can: CAN, effortLabel: 'Reasoning', efforts: claudeEfforts, speeds: twoSpeeds, toggles: thinking, ...model };
}

function codex(model: Pick<Model, 'id' | 'label'> & Partial<Model>): Model {
  return { provider: 'codex', favourite: false, can: CAN, effortLabel: 'Reasoning', efforts: codexEfforts, speeds: threeSpeeds, toggles: [], ...model };
}

export const pickerCatalogue: Catalogue = {
  providers: [
    { id: 'claude-code', name: 'Claude Code', icon: 'sparkles', can: CAN },
    { id: 'codex', name: 'Codex', icon: 'bot', can: CAN },
  ],
  models: [
    claude({ id: 'opus-5', label: 'Opus 5', favourite: true, isNew: true }),
    claude({ id: 'sonnet-5', label: 'Sonnet 5', favourite: true }),
    claude({ id: 'haiku-5', label: 'Haiku 5', efforts: [], speeds: [], toggles: [] }),
    claude({ id: 'opus-4', label: 'Opus 4', legacy: true }),
    claude({ id: 'sonnet-4', label: 'Sonnet 4', legacy: true, speeds: [] }),
    codex({ id: 'gpt-6-codex', label: 'GPT-6 Codex', favourite: true }),
    codex({ id: 'gpt-6-codex-mini', label: 'GPT-6 Codex Mini', speeds: [] }),
    codex({ id: 'gpt-6-codex-preview', label: 'GPT-6 Codex Preview', subLabel: 'Research', can: cannot('Your plan does not include this model.') }),
  ],
  modes: PERMISSION_MODES,
};

export const pickerChoice: Choice = {
  provider: 'claude-code',
  model: 'opus-5',
  effort: 'high',
  speed: 'standard',
  toggles: ['thinking'],
  mode: 'approval-required',
};
