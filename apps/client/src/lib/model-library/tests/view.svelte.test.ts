// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, expect, it } from 'vitest';
import { installAnimations } from '$lib/forms/animations.fixture';
import LibraryView from '../LibraryView.svelte';
import { LibraryStore } from '../store.svelte';
import { fakeLibrary } from './library.fixture';

let app: ReturnType<typeof mount> | undefined;

beforeEach(() => {
  installAnimations();
});

afterEach(async () => {
  if (app !== undefined) await unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

const text = (selector: string) => [...document.querySelectorAll(selector)].map((each) => each.textContent.replace(/\s+/g, ' ').trim());

it('FIT-005: each model has its best fit, and the selected variant has a row with the reason for each machine', async () => {
  const store = new LibraryStore(fakeLibrary());
  app = mount(LibraryView, { target: document.body, props: { store } });
  await store.load();
  store.select('qwen3');
  flushSync();

  expect(text('[aria-label="The models of the library"] li .badge')).toEqual(['Too large', 'Fits on the GPU']);
  expect(text('.variants [aria-checked="true"]')).toEqual(['8b-q4_k_m GGUF · q4_k_m · 4.4 GiB Fits on the GPU']);
  expect(text('.machines li')).toEqual(['nasqwen3:8b-q4_k_m on nasCPU only', 'pop-osqwen3:8b-q4_k_m on pop-osFits on the GPU']);
  expect(text('.head button')).toEqual(['Pull to pop-os']);
});
