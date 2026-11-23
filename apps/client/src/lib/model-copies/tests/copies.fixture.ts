import type { HubError, HubResult } from '$lib/hub/hub';
import type { CopyCalls, FoundModel, ModelCopy, PullChoices } from '../api';

// Test support: copies in the hub's shape, and calls that answer with them and record what the section asked.

const GIB = 1024 * 1024 * 1024;

export function copy(change: Partial<ModelCopy> & Pick<ModelCopy, 'id'>): ModelCopy {
  return {
    place: { kind: 'MACHINE', id: 'm-pop', name: 'pop-os' },
    address: 'qwen3:8b-q4_k_m',
    origin: 'PULLED',
    format: 'GGUF',
    quantisation: 'Q4_K_M',
    repository: 'unsloth/Qwen3-8B-GGUF',
    sizeBytes: 5 * GIB,
    fileCount: 1,
    state: 'DOWNLOADED',
    progress: null,
    reason: null,
    createdAt: '2026-11-23T10:00:00Z',
    ...change,
  };
}

export const COPIES: readonly ModelCopy[] = [
  copy({ id: 'c-pop-qwen', state: 'DOWNLOADING', progress: { bytes: 2 * GIB, totalBytes: 5 * GIB, bytesPerSecond: 10 * 1024 * 1024, secondsLeft: 300 } }),
  copy({ id: 'c-pop-gemma', address: 'gemma3:4b-q8_0', quantisation: 'Q8_0', sizeBytes: 4 * GIB, state: 'VALIDATED' }),
  copy({ id: 'c-nas-qwen', place: { kind: 'MACHINE', id: 'm-nas', name: 'nas' }, state: 'FAILED', reason: 'checksum' }),
];

export function found(change: Partial<FoundModel> & Pick<FoundModel, 'found'>): FoundModel {
  return {
    place: { kind: 'MACHINE', id: 'm-pop', name: 'pop-os' },
    tool: 'OLLAMA',
    name: 'qwen3:8b',
    format: 'GGUF',
    fileCount: 1,
    sizeBytes: 5 * GIB,
    usable: true,
    readAt: '2026-11-23T10:00:00Z',
    ...change,
  };
}

export const FOUND: readonly FoundModel[] = [
  found({ found: 'a'.repeat(64) }),
  found({ found: 'b'.repeat(64), tool: 'HUGGINGFACE', name: 'Qwen/Qwen3-8B/model.safetensors', format: 'SAFETENSORS', usable: false }),
];

export const CHOICES: PullChoices = {
  machines: [
    { id: 'm-nas', name: 'nas', stop: 'QUIET_HOURS' },
    { id: 'm-new', name: 'new', stop: 'NO_SWITCH' },
    { id: 'm-pop', name: 'pop-os', stop: null },
  ],
  variants: [{ address: 'qwen3:8b-q4_k_m', sizeBytes: 5 * GIB }],
};

const ok = <T>(value: T): Promise<HubResult<T>> => Promise.resolve({ ok: true, value });
const refused = <T>(error: HubError): Promise<HubResult<T>> => Promise.resolve({ ok: false, error });

export interface FakeCopies extends CopyCalls {
  copies: ModelCopy[];
  foundModels: FoundModel[];
  readonly finds: string[];
  readonly uses: [string, string][];
  readonly pulls: [string, string][];
  readonly removals: string[];
  listAsked: number;
  /** The answer of the next pull and removal; the default is a new copy and REMOVING. */
  pullFails: HubError | null;
  removeAnswer: HubResult<ModelCopy | null> | null;
}

export function fakeCopies(): FakeCopies {
  const fake: FakeCopies = {
    copies: [...COPIES],
    foundModels: [...FOUND],
    finds: [],
    uses: [],
    pulls: [],
    removals: [],
    listAsked: 0,
    pullFails: null,
    removeAnswer: null,
    list: () => {
      fake.listAsked += 1;
      return ok(fake.copies);
    },
    choices: () => ok(CHOICES),
    pull: (deviceId, address) => {
      fake.pulls.push([deviceId, address]);
      if (fake.pullFails !== null) return refused(fake.pullFails);
      return ok(copy({ id: `c-${deviceId}-new`, place: { kind: 'MACHINE', id: deviceId, name: deviceId }, address, state: 'QUEUED' }));
    },
    found: () => ok(fake.foundModels),
    find: (deviceId) => {
      fake.finds.push(deviceId);
      return ok(`job-${deviceId}`);
    },
    use: (deviceId, key) => {
      fake.uses.push([deviceId, key]);
      const model = fake.foundModels.find((each) => each.found === key);
      if (model === undefined) return refused({ code: 'NOT_FOUND', message: 'The last model report of that machine lists no such model.' });
      return ok(copy({ id: `c-${key.slice(0, 4)}`, place: model.place, address: model.name, origin: 'FOUND', quantisation: '', repository: 'ollama', state: 'QUEUED' }));
    },
    remove: (id) => {
      fake.removals.push(id);
      if (fake.removeAnswer !== null) return Promise.resolve(fake.removeAnswer);
      const found = fake.copies.find((each) => each.id === id);
      return found === undefined ? refused({ code: 'NOT_FOUND', message: 'No such model copy.' }) : ok({ ...found, state: 'REMOVING' as const, progress: null });
    },
  };
  return fake;
}
