// @vitest-environment jsdom
import { flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { installAnimations } from '$lib/forms/animations.fixture';
import { LiveThreads } from '$lib/agent-threads/live/live.svelte';
import { outcomeFrom, type Action } from '$lib/agent-threads/live/run';
import DraftThread from '../draft/DraftThread.svelte';
import { ThreadDraft } from '../draft/thread-draft.svelte';
import MODELS from '../draft/real-models-answer.json';

let app: ReturnType<typeof mount> | undefined;
beforeEach(() => {
  installAnimations();
  Element.prototype.scrollIntoView = vi.fn();
});
afterEach(async () => {
  if (app) await unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

async function settle(): Promise<void> {
  flushSync();
  await tick();
  await tick();
  flushSync();
}
function button(words: string): HTMLButtonElement {
  const found = [...document.querySelectorAll('button')].find((button) => button.textContent.trim() === words);
  if (!found) throw new Error(`No button: ${words}`);
  return found;
}
function enter(label: string, value: string): void {
  const id = [...document.querySelectorAll('label')].find((element) => element.textContent === label)?.htmlFor;
  const field = id ? document.getElementById(id) : null;
  if (!(field instanceof HTMLInputElement)) throw new Error(`No field: ${label}`);
  field.value = value;
  field.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
}
function form(): HTMLFormElement {
  const found = document.querySelector<HTMLFormElement>('[role="dialog"] form');
  if (!found) throw new Error('No form');
  return found;
}
async function show(create: (action: Action) => Promise<unknown>, registered: unknown[] = [], source = 'New project') {
  const calls: Action[] = [];
  const live = new LiveThreads({
    call: async (action) => {
      calls.push(action);
      const wire = action.call === 'models' ? MODELS : action.call === 'projects' ? { outcome: 'projects', projects: registered }
        : action.call === 'folders' ? { outcome: 'folders', folder: '/real/projects', folders: [], more: false } : await create(action);
      const outcome = outcomeFrom(wire);
      if (outcome === null) throw new Error('The answer did not read.');
      return outcome;
    },
    watch: async () => () => undefined,
    notify: async () => undefined,
  }, () => ({ front: true, chosen: null }));
  const draft = new ThreadDraft(null).addProject();
  app = mount(DraftThread, { target: document.body, props: { draft, live, projects: [], onStarted: vi.fn() } });
  await settle();
  const row = [...document.querySelectorAll<HTMLElement>('[role="option"]')].find((option) => option.textContent.includes(source));
  row?.click();
  await settle();
  return { calls, draft };
}

it('creates once through the runner and selects only its confirmed canonical project', async () => {
  let finish!: (wire: unknown) => void;
  const { calls, draft } = await show(() => new Promise((resolve) => { finish = resolve; }));
  enter('Name', 'Field notes');
  enter('Location', '/alias/projects');
  button('Create project').click();
  await settle();
  expect(calls.filter((action) => action.call === 'create_project')).toEqual([{ call: 'create_project', folder: '/real/projects/Field notes', name: 'Field notes' }]);
  expect(draft.folder).toBeNull();
  expect(form().getAttribute('aria-busy')).toBe('true');
  form().dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
  button('Cancel').click();
  await settle();
  expect(document.querySelector('[role="dialog"]')).not.toBeNull();
  expect(calls.filter((action) => action.call === 'create_project')).toHaveLength(1);
  finish({ outcome: 'projects', projects: [{ folder: '/real/projects/Field notes', name: 'Field notes' }] });
  await settle();
  await vi.waitFor(() => expect(draft.folder).toBe('/real/projects/Field notes'));
  flushSync();
  expect(document.querySelector('h1')?.textContent).toContain('Field notes');
  expect(calls.some((action) => action.call === 'start')).toBe(false);
});

it('keeps inputs and shows the runner refusal, then permits a successful retry', async () => {
  let attempts = 0;
  const { draft } = await show(async () => ++attempts === 1
    ? { outcome: 'refused', why: { refusal: 'cannot_create_project', said: 'That folder already exists. Choose another name.' } }
    : { outcome: 'projects', projects: [{ folder: '/real/projects/Other', name: 'Other' }] });
  enter('Name', 'Existing');
  enter('Location', '/real/projects');
  button('Create project').click();
  await settle();
  await vi.waitFor(() => expect(document.body.textContent).toContain('That folder already exists. Choose another name.'));
  expect([...document.querySelectorAll('input')].map((input) => input.value)).toContain('Existing');
  expect(draft.folder).toBeNull();
  enter('Name', 'Other');
  button('Create project').click();
  await settle();
  await vi.waitFor(() => expect(draft.folder).toBe('/real/projects/Other'));
});

it('refuses invalid fields locally and never selects an unconfirmed or malformed result', async () => {
  const { calls, draft } = await show(async () => ({ outcome: 'projects', projects: [{ folder: 42, name: 'Bad' }] }));
  enter('Name', '../escape');
  enter('Location', 'relative');
  button('Create project').click();
  await settle();
  expect(calls.some((action) => action.call === 'folders' || action.call === 'create_project')).toBe(false);
  expect(document.querySelectorAll('[aria-invalid="true"]')).toHaveLength(2);
  enter('Name', 'Notes');
  enter('Location', '/real/projects');
  button('Create project').click();
  await settle();
  expect(draft.folder).toBeNull();
  await vi.waitFor(() => expect(document.body.textContent).toContain('The answer did not read.'));
});

it('loads a saved empty project into the picker after reopening', async () => {
  await show(async () => ({ outcome: 'ok' }), [{ folder: '/Users/ana/Notes', name: 'Saved notes' }]);
  button('Cancel').click();
  await settle();
  button('No project').click();
  await settle();
  expect(document.body.textContent).toContain('Saved notes');
});


it.each([{ outcome: 'ok' }, { outcome: 'projects', projects: [] }])('does not select a project without its registry confirmation: %j', async (wire) => {
  const { draft } = await show(async () => wire);
  enter('Name', 'Notes');
  enter('Location', '/real/projects');
  button('Create project').click();
  await vi.waitFor(() => expect(document.body.textContent).toContain('Creation was not confirmed.'));
  expect(draft.folder).toBeNull();
  expect(document.querySelector('[role="dialog"]')).not.toBeNull();
});

it('does not select a late creation after leaving the draft', async () => {
  let finish!: (wire: unknown) => void;
  const { calls, draft } = await show(() => new Promise((resolve) => { finish = resolve; }));
  enter('Name', 'Notes');
  enter('Location', '/real/projects');
  button('Create project').click();
  await vi.waitFor(() => expect(calls.some((action) => action.call === 'create_project')).toBe(true));
  if (app) await unmount(app);
  app = undefined;
  finish({ outcome: 'projects', projects: [{ folder: '/real/projects/Notes', name: 'Notes' }] });
  await settle();
  expect(draft.folder).toBeNull();
});


it('explains how to recover when the creation acknowledgement is lost', async () => {
  const { draft } = await show(async () => { throw new Error('The runner disconnected.'); });
  enter('Name', 'Notes');
  enter('Location', '/real/projects');
  button('Create project').click();
  await vi.waitFor(() => expect(document.body.textContent).toContain('The runner disconnected.'));
  expect(document.body.textContent).toContain('Check the location before retrying');
  expect(draft.folder).toBeNull();
  expect([...document.querySelectorAll('input')].map((input) => input.value)).toContain('Notes');
});

it('clones the Git URL into the confirmed canonical folder and selects the registered project', async () => {
  const { calls, draft } = await show(async () => ({ outcome: 'projects', projects: [{ folder: '/real/projects/Notes', name: 'Notes' }] }), [], 'Git URL');
  const url = 'https://git.example.test/team/notes.git';
  enter('Git URL', url);
  enter('Name', 'Notes');
  enter('Location', '/alias/projects');
  button('Clone project').click();
  await vi.waitFor(() => expect(draft.folder).toBe('/real/projects/Notes'));
  expect(calls.filter((action) => action.call === 'clone_project')).toEqual([
    { call: 'clone_project', folder: '/real/projects/Notes', name: 'Notes', url },
  ]);
  expect(calls.some((action) => action.call === 'create_project' || action.call === 'start')).toBe(false);
});
