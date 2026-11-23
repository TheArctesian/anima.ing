import type { HubError, HubResult } from '$lib/hub/hub';
import type { LibraryCalls, LibraryModel, ModelFit } from '../api';

const variant = (tag: string, sizeBytes: number, format: 'GGUF' | 'MLX' = 'GGUF') => ({
  address: `qwen3:${tag}`,
  tag,
  format,
  quantisation: tag.split('-').pop() ?? '',
  sizeBytes,
});

export const QWEN: LibraryModel = {
  name: 'qwen3',
  family: 'qwen3',
  parameterCount: 8.2e9,
  contextLength: 32768,
  capabilities: ['CHAT', 'TOOLS', 'IMAGE_UNDERSTANDING'],
  licence: 'Apache-2.0',
  publisher: 'Qwen',
  aliases: ['coder'],
  variants: [variant('8b-q8_0', 8.1e9), variant('8b-q4_k_m', 4.7e9), variant('8b-4bit', 4.3e9, 'MLX')],
};

export const LLAMA: LibraryModel = {
  name: 'llama3.3',
  family: null,
  parameterCount: 70e9,
  contextLength: 131072,
  capabilities: ['CHAT'],
  licence: null,
  publisher: 'Meta',
  aliases: [],
  variants: [{ address: 'llama3.3:70b-q4_k_m', tag: '70b-q4_k_m', format: 'GGUF', quantisation: 'Q4_K_M', sizeBytes: 42e9 }],
};

type Shape = Pick<ModelFit, 'state'> & Partial<ModelFit>;

/** One fit of `address` on the machine `machine`. */
export function fit(address: string, machine: string, shape: Shape): ModelFit {
  return {
    variant: address.split(':')[1] ?? '',
    address,
    place: { kind: 'MACHINE', id: `m-${machine}`, name: machine },
    placement: null,
    backend: null,
    contextTokens: 8192,
    largestContext: null,
    sizeBytes: 1,
    neededBytes: null,
    usableBytes: null,
    reason: `${address} on ${machine}`,
    ...shape,
  };
}

const FITS: Readonly<Record<string, readonly ModelFit[]>> = {
  qwen3: [
    fit('qwen3:8b-q8_0', 'pop-os', { state: 'REDUCED_CONTEXT', placement: 'GPU', largestContext: 16384 }),
    fit('qwen3:8b-q4_k_m', 'pop-os', { state: 'FITS', placement: 'GPU' }),
    fit('qwen3:8b-q4_k_m', 'nas', { state: 'FITS', placement: 'CPU' }),
    fit('qwen3:8b-4bit', 'pop-os', { state: 'UNKNOWN' }),
  ],
  'llama3.3': [fit('llama3.3:70b-q4_k_m', 'pop-os', { state: 'TOO_LARGE', placement: 'GPU' })],
};

/** Fake library calls that record the contexts and the pulls. */
export function fakeLibrary() {
  const calls = {
    contexts: [] as (number | null)[],
    pulls: [] as [string, string][],
    pullFails: null as HubError | null,
    models: (): Promise<HubResult<readonly LibraryModel[]>> => Promise.resolve({ ok: true, value: [LLAMA, QWEN] }),
    fits: (model: string, context: number | null): Promise<HubResult<readonly ModelFit[]>> => {
      calls.contexts.push(context);
      return Promise.resolve({ ok: true, value: FITS[model] ?? [] });
    },
    pull: (deviceId: string, address: string): Promise<HubResult<unknown>> => {
      calls.pulls.push([deviceId, address]);
      return Promise.resolve(calls.pullFails === null ? { ok: true, value: null } : { ok: false, error: calls.pullFails });
    },
  };
  return calls satisfies LibraryCalls;
}
