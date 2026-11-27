// @vitest-environment jsdom
import { flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, expect, it, vi } from 'vitest';
import { fakeHub } from '$lib/devices/hub.fixture';
import { endTransitionsAtOnce } from '$lib/devices/transitions.fixture';
import { nodeCodeApi } from '../api';
import Workspaces from './Workspaces.svelte';

endTransitionsAtOnce();
const device = '0190a000-0000-7000-8002-000000000002';
const workspace = '0190a000-0000-7000-8002-000000000003';
let app: ReturnType<typeof mount> | undefined;

afterEach(async () => {
  if (app) await unmount(app);
  app = undefined;
  localStorage.clear();
  document.body.replaceChildren();
});

function button(label: string, icon = false): HTMLButtonElement {
  const found = [...document.querySelectorAll('button')].find((element) =>
    icon ? element.getAttribute('aria-label') === label : element.textContent?.trim() === label);
  if (!found) throw new Error(`Missing button: ${label}`);
  return found;
}

it('confirms deletion of a never-started workspace and retains the exact request after a lost reply', async () => {
  const fake = fakeHub({ CodeWorkspaceInventory: { managedNodeWorkspaces: {
    device, available: true, workspaces: [{ workspace,
      request: '0190a000-0000-7000-8002-000000000004', state: 'completed',
      action: { kind: 'prepare', storage_bytes: 2 ** 30 },
      receipt: { result: { workspace, generation: 0, phase: 'prepared' } },
    }],
  } } });
  app = mount(Workspaces, { target: document.body, props: {
    api: nodeCodeApi(fake.hub), device, origin: fake.hub.address.origin,
    storage: localStorage, onuse: vi.fn(),
  } });
  flushSync();
  await vi.waitFor(() => expect(button('Delete workspace', true).disabled).toBe(false));
  button('Delete workspace', true).click();
  await tick();
  const mutations = () => fake.requests.filter((request) => request.operation === 'CodeWorkspaceSubmit');
  expect(mutations()).toHaveLength(0);
  expect(document.body.textContent).toContain('Permanently delete this workspace and its files?');
  button('Cancel').click();
  await tick();
  expect(mutations()).toHaveLength(0);
  button('Delete workspace', true).click();
  await tick();
  button('Delete workspace').click();
  await vi.waitFor(() => expect(mutations()).toHaveLength(1));
  const original = mutations()[0]?.variables;
  expect(original).toMatchObject({ device, input: { workspace,
    action: { kind: 'delete', generation: 0, confirmed: true },
  } });
  await vi.waitFor(() => expect(button('Retry same request').disabled).toBe(false));
  expect(button('New workspace', true).disabled).toBe(true);
  const retained = localStorage.getItem(`anima-workspace-request:${fake.hub.address.origin}:${device}`);
  expect(retained).toContain('"confirmed":true');
  button('Retry same request').click();
  await vi.waitFor(() => expect(mutations()).toHaveLength(2));
  expect(mutations()[1]?.variables).toEqual(original);
});

it('submits the exact public checkout and keeps it on an unconfirmed request', async () => {
  const fake = fakeHub({ CodeWorkspaceInventory: { managedNodeWorkspaces: {
    device, available: true, workspaces: [],
  } } });
  app = mount(Workspaces, { target: document.body, props: {
    api: nodeCodeApi(fake.hub), device, origin: fake.hub.address.origin,
    storage: localStorage, onuse: vi.fn(),
  } });
  await vi.waitFor(() => expect(button('New workspace', true).disabled).toBe(false));
  button('New workspace', true).click();
  await tick();
  const [repository, commit] = document.querySelectorAll('form input');
  for (const [input, value] of [[repository, 'example/app'], [commit, 'ABCDEF0123'.repeat(4)]] as const) {
    if (!(input instanceof HTMLInputElement)) throw new Error('Missing checkout input');
    input.value = value;
    input.dispatchEvent(new Event('input', { bubbles: true }));
  }
  button('Prepare workspace').click();
  const mutations = () => fake.requests.filter((request) => request.operation === 'CodeWorkspaceSubmit');
  await vi.waitFor(() => expect(mutations()).toHaveLength(1));
  expect(mutations()[0]?.variables).toMatchObject({
    device, input: { action: { kind: 'prepare', storage_bytes: 8 * 2 ** 30,
      source: { owner: 'example', repository: 'app', commit: 'abcdef0123'.repeat(4) },
    } },
  });
  await vi.waitFor(() => expect(button('Retry same request').disabled).toBe(false));
  button('Retry same request').click();
  await vi.waitFor(() => expect(mutations()).toHaveLength(2));
  expect(mutations()[1]?.variables).toEqual(mutations()[0]?.variables);
});

it('retains the connected private repository identity when preparation is retried', async () => {
  const repositoryId = '0190a000-0000-7000-8002-000000000005';
  const fake = fakeHub({
    CodeWorkspaceInventory: { managedNodeWorkspaces: {
      device, available: true, privateCheckout: true, workspaces: [],
    } },
    CodeWorkspaceRepositories: { projects: {
      edges: [{ node: { repositories: [
        { id: repositoryId, fullName: 'example/internal', private: true, access: 'READY' },
      ] } }],
      pageInfo: { hasNextPage: false, endCursor: null },
    } },
  });
  app = mount(Workspaces, { target: document.body, props: {
    api: nodeCodeApi(fake.hub), device, origin: fake.hub.address.origin,
    storage: localStorage, onuse: vi.fn(),
  } });
  const select = (label: string): HTMLSelectElement => {
    const control = [...document.querySelectorAll('label')]
      .find((entry) => entry.firstChild?.textContent?.trim() === label)?.control;
    if (!(control instanceof HTMLSelectElement)) throw new Error(`Missing select: ${label}`);
    return control;
  };
  const choose = (label: string, value: string): void => {
    const control = select(label);
    control.value = value;
    control.dispatchEvent(new Event('change', { bubbles: true }));
  };
  await vi.waitFor(() => expect(button('New workspace', true).disabled).toBe(false));
  button('New workspace', true).click();
  await tick();
  choose('Repository access', 'private');
  await vi.waitFor(() => expect(select('Connected private repository').disabled).toBe(false));
  choose('Connected private repository', repositoryId);
  const commit = document.querySelector('form input');
  if (!(commit instanceof HTMLInputElement)) throw new Error('Missing commit input');
  commit.value = 'ab'.repeat(20);
  commit.dispatchEvent(new Event('input', { bubbles: true }));
  button('Prepare workspace').click();
  const mutations = () => fake.requests.filter(({ operation }) => operation === 'CodeWorkspaceSubmit');
  await vi.waitFor(() => expect(button('Retry same request').disabled).toBe(false));
  expect(mutations()[0]?.variables).toMatchObject({ input: { action: { source: {
    owner: 'example', repository: 'internal', commit: 'ab'.repeat(20), repository_id: repositoryId,
  } } } });
  button('Retry same request').click();
  await vi.waitFor(() => expect(mutations()).toHaveLength(2));
  expect(mutations()[1]?.variables).toEqual(mutations()[0]?.variables);
});

it('holds new workspace work at zero headroom and allows deletion', async () => {
  const fake = fakeHub({ CodeWorkspaceInventory: { managedNodeWorkspaces: {
    device, available: true, capacity: { storageBytes: 0, memoryBytes: 0, cpuMillis: 0, tasks: 0 },
    workspaces: [{ workspace, request: '0190a000-0000-7000-8002-000000000004', state: 'completed',
      action: { kind: 'prepare', storage_bytes: 2 ** 30 },
      receipt: { result: { workspace, generation: 0, phase: 'prepared' } },
    }],
  } } });
  app = mount(Workspaces, { target: document.body, props: {
    api: nodeCodeApi(fake.hub), device, origin: fake.hub.address.origin,
    storage: localStorage, onuse: vi.fn(),
  } });
  await vi.waitFor(() => expect(button('Delete workspace', true).disabled).toBe(false));
  expect(button('Start workspace').disabled).toBe(true);
  button('New workspace', true).click();
  await tick();
  expect(button('Prepare workspace').disabled).toBe(true);
  expect(document.body.textContent).toContain('Choose a smaller storage limit or another node.');
  expect(button('Delete workspace', true).disabled).toBe(false);
});
