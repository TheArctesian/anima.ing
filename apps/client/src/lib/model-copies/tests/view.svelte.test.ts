// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, expect, it } from 'vitest';
import { installAnimations } from '$lib/forms/animations.fixture';
import CopyList from '../CopyList.svelte';
import FoundList from '../FoundList.svelte';
import { CopiesStore } from '../store.svelte';
import { fakeCopies, type FakeCopies } from './copies.fixture';

let app: ReturnType<typeof mount> | undefined;
let calls: FakeCopies;

beforeEach(() => {
  installAnimations();
  calls = fakeCopies();
});

afterEach(async () => {
  if (app !== undefined) await unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

async function open(): Promise<CopiesStore> {
  const store = new CopiesStore(calls);
  app = mount(CopyList, { target: document.body, props: { store } });
  await store.load();
  flushSync();
  return store;
}

const text = (selector: string) => [...document.querySelectorAll(selector)].map((each) => each.textContent.replace(/\s+/g, ' ').trim());

it('HSE-013 and HSE-014: one group for each machine, and each row has its model, its state and its progress', async () => {
  await open();
  expect(text('tr.group th')).toEqual(['nas 1 model · 5.0 GiB', 'pop-os 2 models · 9.0 GiB']);
  expect(text('tbody th[scope="row"]')).toEqual(['qwen3:8b-q4_k_m GGUF · Q4_K_M · 1 file · 5.0 GiB', 'gemma3:4b-q8_0 GGUF · Q8_0 · 1 file · 4.0 GiB', 'qwen3:8b-q4_k_m GGUF · Q4_K_M · 1 file · 5.0 GiB']);
  expect(text('tbody td:nth-of-type(1)')).toEqual(['Failed A file did not match its SHA-256 value.', 'Ready', 'Downloading 40 % · 2.0 GiB of 5.0 GiB · 10.0 MiB/s · 5 min left']);
  expect(document.querySelector('progress')?.getAttribute('value')).toBe('40');
});

it('PULL-009: Remove asks first, then the row says Removing and offers no second removal', async () => {
  await open();
  document.querySelector<HTMLButtonElement>('button[aria-label="Remove gemma3:4b-q8_0 from pop-os"]')?.click();
  flushSync();
  const confirm = [...document.querySelectorAll<HTMLButtonElement>('[role="dialog"] button, [role="alertdialog"] button')].find((button) => button.textContent.trim() === 'Remove');
  confirm?.click();
  await Promise.resolve();
  await Promise.resolve();
  flushSync();
  expect(calls.removals).toEqual(['c-pop-gemma']);
  expect(text('tbody td:nth-of-type(1)')).toContain('Removing');
  expect(document.querySelector('button[aria-label="Remove gemma3:4b-q8_0 from pop-os"]')).toBeNull();
});

it('HSE-013: a hub with no copy says so and how to get one', async () => {
  calls.copies = [];
  await open();
  expect(text('p.empty')[0]).toContain('No machine has a model.');
});

async function openFound(): Promise<CopiesStore> {
  const store = new CopiesStore(calls);
  app = mount(FoundList, { target: document.body, props: { store } });
  await store.load();
  flushSync();
  return store;
}

it('PULL-012: each found model says where it is, a GGUF model has Use, and Use asks for that model', async () => {
  await openFound();
  expect(text('tbody th[scope="row"]')).toEqual(['Qwen/Qwen3-8B/model.safetensors Hugging Face · SAFETENSORS · 1 file · 5.0 GiB', 'qwen3:8b Ollama · GGUF · 1 file · 5.0 GiB']);
  expect(text('td.action')).toEqual(['GGUF only', 'Use']);
  document.querySelector<HTMLButtonElement>('button[aria-label="Use qwen3:8b on pop-os where it is"]')?.click();
  await Promise.resolve();
  await Promise.resolve();
  flushSync();
  expect(calls.uses).toEqual([['m-pop', 'a'.repeat(64)]]);
  expect(text('tbody th[scope="row"]')).toEqual(['Qwen/Qwen3-8B/model.safetensors Hugging Face · SAFETENSORS · 1 file · 5.0 GiB']);
});
