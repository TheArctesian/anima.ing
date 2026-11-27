// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { installAnimations } from '$lib/forms/animations.fixture';
import { fakeHosting } from '../hosting.fixture';
import ComposeApply from './ComposeApply.svelte';
import type { ApplyStatus } from './api';

let mounted: ReturnType<typeof mount> | undefined;

beforeEach(installAnimations);

afterEach(async () => {
  if (mounted !== undefined) await unmount(mounted);
  mounted = undefined;
  document.body.replaceChildren();
  vi.useRealTimers();
});

function button(label: string): HTMLButtonElement {
  const found = [...document.querySelectorAll('button')].find((element) => element.textContent.trim() === label);
  if (found === undefined) throw new Error(`Missing button ${label}`);
  return found;
}

function text(field: HTMLTextAreaElement | HTMLInputElement, value: string): void {
  field.value = value;
  field.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
}

it('queues once on the selected device, exposes secret_failed, and opens a fresh form bound to the same application', async () => {
  vi.useFakeTimers();
  const apply: ApplyStatus = { id: 'attempt-1', applicationId: 'app-1', deviceId: 'm-pop', state: 'queued', reason: null, health: null };
  const queue = vi.fn(async () => ({ ok: true as const, value: apply }));
  const status = vi.fn(async () => ({ ok: true as const, value: { ...apply, state: 'failed', reason: 'secret_failed' } }));
  const calls = { ...fakeHosting().apply, queue, status };
  mounted = mount(ComposeApply, { target: document.body, props: { calls, onlyId: 'm-pop', preferredId: 'm-pop', onApplied: vi.fn() } });

  button('Deploy Compose').click();
  await Promise.resolve();
  flushSync();
  const fields = document.querySelectorAll<HTMLTextAreaElement>('textarea');
  expect(document.querySelector('select')?.value).toBe('m-pop');
  const [composeFile, environmentFile] = fields;
  if (composeFile === undefined || environmentFile === undefined) throw new Error('Missing apply editors');
  text(composeFile, 'services:\n  web:\n    image: example/web:1');
  text(environmentFile, 'TOKEN=synthetic');
  button('Queue apply').click();
  await Promise.resolve();
  flushSync();

  expect(queue).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ deviceId: 'm-pop', environmentFile: 'TOKEN=synthetic' }));
  expect(document.body.textContent).toContain('Queued for the device.');
  await vi.advanceTimersByTimeAsync(2500);
  flushSync();
  expect(status).toHaveBeenCalledExactlyOnceWith('attempt-1');
  expect(document.body.textContent).toContain('Submit a fresh apply');
  button('Prepare fresh apply').click();
  await Promise.resolve();
  flushSync();
  expect(document.querySelector<HTMLInputElement>('input')?.value).toBe('app-1');
  expect(document.querySelectorAll<HTMLTextAreaElement>('textarea')[1]?.value).toBe('');
  expect(queue).toHaveBeenCalledTimes(1);
});
