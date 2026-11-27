import type { BuildInput, BuildReceipt } from './api';

export const limit = 12;
export const strategyWords = { AUTO: 'Auto detect', DOCKERFILE: 'Dockerfile', RAILPACK: 'Railpack' } as const;
export const targetWords = { LINUX_X86_64: 'Linux x86-64', LINUX_AARCH64: 'Linux ARM64' } as const;

export interface BuildIntent { input: BuildInput; job: string | null }

function object(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value) ? value as Record<string, unknown> : null;
}

export function identity(value: unknown): value is string {
  return typeof value === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/iu.test(value)
    && value !== '00000000-0000-0000-0000-000000000000';
}

export function options(value: unknown): Omit<BuildInput, 'request'> | null {
  const input = object(value);
  if (!input || !identity(input.repository) || typeof input.commit !== 'string' || !/^[0-9a-f]{40}$/iu.test(input.commit)) return null;
  const { target, strategy, memoryMib } = input;
  if (target !== 'LINUX_X86_64' && target !== 'LINUX_AARCH64') return null;
  if (strategy !== 'AUTO' && strategy !== 'DOCKERFILE' && strategy !== 'RAILPACK') return null;
  if (typeof memoryMib !== 'number' || !Number.isInteger(memoryMib) || memoryMib < 512 || memoryMib > 2048) return null;
  return { repository: input.repository, commit: input.commit.toLowerCase(), target, strategy, memoryMib };
}

function readIntent(value: unknown): BuildIntent | null {
  const row = object(value);
  const input = object(row?.input);
  const request = input?.request;
  const selection = options(input);
  if (!row || !identity(request) || !selection || !(row.job === null || identity(row.job))) return null;
  return { input: { request, ...selection }, job: row.job };
}

export function load(storage: Storage | null, key: string): BuildIntent[] {
  try {
    const raw = storage?.getItem(key);
    if (!raw || raw.length > 8192) return [];
    const values: unknown = JSON.parse(raw);
    if (!Array.isArray(values) || values.length > limit) return [];
    const unique = new Map<string, BuildIntent>();
    for (const value of values as unknown[]) {
      const intent = readIntent(value);
      if (intent !== null) unique.set(intent.input.request, intent);
    }
    return [...unique.values()];
  } catch { return []; }
}

export function save(storage: Storage | null, key: string, values: BuildIntent[]): boolean {
  if (!storage) return false;
  try { storage.setItem(key, JSON.stringify(values)); return true; }
  catch { return false; }
}

const labels: Record<string, string> = {
  queued: 'Queued', claimed: 'Assigned', preparing: 'Preparing source',
  running: 'Building image', finishing: 'Verifying and retaining image',
  done: 'Image ready', failed: 'Build failed', stopped: 'Stopped',
};

export function stateWords(build: BuildReceipt | undefined): string {
  return build ? labels[build.state] ?? 'Status unavailable' : 'Checking build status…';
}
export function finished(build: BuildReceipt | undefined): boolean {
  return build !== undefined && ['done', 'stopped', 'failed'].includes(build.state);
}
