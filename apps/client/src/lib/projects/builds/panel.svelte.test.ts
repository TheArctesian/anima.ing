// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte';
import { afterEach, expect, it, vi } from 'vitest';
import { fakeHub } from '$lib/devices/hub.fixture';
import { repository } from '../projects.fixture';
import { sourceBuildApi } from './api';
import SourceBuildPanel from './SourceBuildPanel.svelte';

const repo = '0190a000-0000-7000-8002-000000000002';
const job = '0190a000-0000-7000-8002-000000000003';
const privateRepo = '0190a000-0000-7000-8002-000000000004';
const builder = { id: 'sample-node', name: 'Workshop', target: 'LINUX_AARCH64', privateCheckout: true };
const commit = 'ab'.repeat(20);
const project = { id: 'sample-project', repositories: [
  repository(repo, 'sample/app'),
  repository(privateRepo, 'sample/internal', { private: true }),
] };
let app: ReturnType<typeof mount> | undefined;

afterEach(async () => {
  if (app !== undefined) await unmount(app);
  app = undefined;
  localStorage.clear();
  document.body.replaceChildren();
});

function button(label: string): HTMLButtonElement {
  const entry = [...document.querySelectorAll('button')].find((element) =>
    element.textContent?.trim() === label || element.getAttribute('aria-label') === label);
  if (!entry) throw new Error(`Missing button ${label}`);
  return entry;
}

function enterCommit(): void {
  const input = document.querySelector('form input[type="text"]');
  if (!(input instanceof HTMLInputElement)) throw new Error('Missing commit input');
  input.value = commit;
  input.dispatchEvent(new Event('input', { bubbles: true }));
}

function choose(label: string, value: string): void {
  const field = [...document.querySelectorAll('label')].find((element) => element.textContent?.trim() === label);
  if (!(field?.control instanceof HTMLSelectElement)) throw new Error(`Missing select ${label}`);
  field.control.value = value;
  field.control.dispatchEvent(new Event('change', { bubbles: true }));
}

it('recovers the same public build request after a lost reply and shows its accepted job', async () => {
  const answers: Record<string, unknown> = {
    ProjectBuildPlacement: { projectBuildPlacement: { pinned: null, candidates: [] } },
  };
  const fake = fakeHub(answers);
  const api = sourceBuildApi(fake.hub);
  const open = () => mount(SourceBuildPanel, { target: document.body, props: { api, project } });
  const mutations = () => fake.requests.filter(({ operation }) => operation === 'QueueProjectSourceBuild');
  app = open();

  button('New build').click();
  await tick();
  enterCommit();
  await tick();
  await vi.waitFor(() => expect(button('Queue build').disabled).toBe(false));
  button('Queue build').click();
  await vi.waitFor(() => expect(button('Recover build request').getAttribute('aria-busy')).toBeNull());
  const original = mutations()[0]?.variables;
  expect(original).toMatchObject({ input: {
    repository: repo, commit, target: 'LINUX_X86_64', strategy: 'AUTO', memoryMib: 2048,
  } });
  expect(mutations()).toHaveLength(1);

  await unmount(app);
  app = open();
  await tick();
  expect(mutations()).toHaveLength(1);
  answers['QueueProjectSourceBuild'] = { queueSourceBuild: { id: job, state: 'queued', attempt: 0 } };
  button('Recover build request').click();
  await vi.waitFor(() => expect(document.body.textContent).toContain(job));
  expect(mutations()).toHaveLength(2);
  expect(mutations()[1]?.variables).toEqual(original);

  answers['ProjectSourceBuildStatus'] = { sourceBuild: { id: job, state: 'done', attempt: 1 } };
  button('Refresh builds').click();
  await vi.waitFor(() => expect(document.body.textContent).toContain('Image ready'));
  expect(fake.requests.map(({ operation }) => operation).filter((operation) => operation !== 'ProjectBuildPlacement')).toEqual([
    'QueueProjectSourceBuild', 'QueueProjectSourceBuild', 'ProjectSourceBuildStatus',
  ]);
});

it('queues a private repository only after saving a capable node pin and uses its architecture', async () => {
  const fake = fakeHub({
    ProjectBuildPlacement: { projectBuildPlacement: { pinned: null, candidates: [builder] } },
    SetProjectBuildPin: { setProjectBuildPin: { pinned: builder, candidates: [builder] } },
    QueueProjectSourceBuild: { queueSourceBuild: { id: job, state: 'queued', attempt: 0 } },
    ProjectSourceBuildStatus: { sourceBuild: { id: job, state: 'failed', attempt: 1, reason: 'limit_memory' } },
  });
  app = mount(SourceBuildPanel, { target: document.body, props: { api: sourceBuildApi(fake.hub), project } });
  button('New build').click();
  await vi.waitFor(() => expect(document.body.textContent).toContain('Workshop'));
  choose('Repository', privateRepo);
  enterCommit();
  await tick();
  expect(button('Queue build').disabled).toBe(true);

  choose('Build node', builder.id);
  await tick();
  expect(button('Queue build').disabled).toBe(true);
  button('Save pin').click();
  await vi.waitFor(() => expect(button('Queue build').disabled).toBe(false));
  button('Queue build').click();
  await vi.waitFor(() => expect(document.body.textContent).toContain(job));
  expect(fake.requests.find(({ operation }) => operation === 'SetProjectBuildPin')?.variables).toEqual({
    project: project.id, device: builder.id,
  });
  expect(fake.requests.find(({ operation }) => operation === 'QueueProjectSourceBuild')?.variables).toMatchObject({
    input: { repository: privateRepo, target: 'LINUX_AARCH64', commit },
  });
  button('Refresh builds').click();
  await vi.waitFor(() => expect(document.body.textContent).toContain('The build ran out of memory.'));
});
