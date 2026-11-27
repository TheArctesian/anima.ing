// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte';
import { afterEach, expect, it, vi } from 'vitest';
import { fakeHub } from '$lib/devices/hub.fixture';
import { endTransitionsAtOnce } from '$lib/devices/transitions.fixture';
import { nodeCodeApi } from '../api';
import Workspaces from './Workspaces.svelte';
import type { Intent } from './records';

endTransitionsAtOnce();

const source = '0190a000-0000-7000-8002-000000000001';
const destination = '0190a000-0000-7000-8002-000000000002';
const workspace = '0190a000-0000-7000-8002-000000000003';
const request = '0190a000-0000-7000-8002-000000000004';
const quota = 2 ** 30;
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

async function openMoveForm(): Promise<void> {
  await vi.waitFor(() => expect(button('Move workspace', true).disabled).toBe(false));
  button('Move workspace', true).click();
  await tick();
}

function selectDestination(value: string): void {
  const select = [...document.querySelectorAll('select')].find((element) =>
    element.closest('label')?.textContent?.includes('Destination node'));
  if (!select) throw new Error('Missing destination selector');
  select.value = value;
  select.dispatchEvent(new Event('change', { bubbles: true }));
}

it('recovers the destination, continuation choice and preview port after an uncertain move reply', async () => {
  const ready = { available: true, capacity: { storageBytes: quota, memoryBytes: quota, cpuMillis: 1000, tasks: 256 } };
  const sourceInventory = { managedNodeWorkspaces: {
    device: source, ...ready, workspaces: [{
      workspace, request, state: 'completed', action: { kind: 'reconcile', generation: 7 },
      receipt: { result: { workspace, generation: 7, phase: 'running', storageBytes: quota } },
    }],
  } };
  const answers: Record<string, unknown> = { CodeWorkspaceInventory: sourceInventory };
  const { hub, requests } = fakeHub(answers);
  const onmoved = vi.fn();
  app = mount(Workspaces, { target: document.body, props: {
    api: nodeCodeApi(hub), device: source, origin: hub.address.origin, storage: localStorage,
    devices: [{ id: destination, name: 'Builder' }], onuse: vi.fn(), onmoved,
    selectedPreview: { device: source, workspace, generation: 7, port: 4321 },
  } });

  await openMoveForm();
  const heldCapacity = { storageBytes: 0, memoryBytes: 0, cpuMillis: 0, tasks: 0 };
  answers.CodeWorkspaceInventory = { managedNodeWorkspaces: {
    device: destination, available: true, capacity: heldCapacity, workspaces: [],
  } };
  selectDestination(destination);
  await vi.waitFor(() => expect(document.body.textContent).toContain('This node has insufficient workspace storage.'));
  expect(button('Move workspace').disabled).toBe(true);

  const continuation = document.querySelector<HTMLInputElement>('label.workspace-check input');
  expect(continuation?.checked).toBe(false);
  continuation?.click();
  await tick();
  expect(button('Move workspace').disabled).toBe(false);
  answers.CodeWorkspaceInventory = sourceInventory;
  button('Move workspace').click();
  const sent = () => requests.find((entry) => entry.operation === 'CodeWorkspaceSubmit');
  await vi.waitFor(() => expect(sent()).toBeDefined());
  expect(sent()?.variables).toMatchObject({ device: source, input: { workspace,
    action: { kind: 'move', generation: 7, storage_bytes: quota, destination_device: destination, continue_session: true },
  } });
  const key = `anima-workspace-request:${hub.address.origin}:${source}`;
  const pending = JSON.parse(localStorage.getItem(key) ?? 'null') as Intent;
  expect(pending.action).toMatchObject({ continue_session: true });
  expect(pending.previewPort).toBe(4321);
  expect(sent()?.variables).not.toHaveProperty('input.previewPort');
  const receipt = { transfer: pending.request, workspace, sourceDevice: source };

  answers.CodeWorkspaceReceipt = { managedWorkspaceRequest: { ...receipt, destinationDevice: destination, state: 'waiting-placement' } };
  await vi.waitFor(() => expect(button('Refresh workspaces', true).disabled).toBe(false));
  button('Refresh workspaces', true).click();
  await vi.waitFor(() => expect(document.body.textContent).toContain('Waiting for capacity on the selected node.'));
  await vi.waitFor(() => expect(requests.filter((entry) => entry.operation === 'CodeWorkspaceInventory')).toHaveLength(3));
  expect(button('Withdraw move').disabled).toBe(false);

  answers.CodeWorkspaceReceipt = { managedWorkspaceRequest: { ...receipt, destinationDevice: source, state: 'activated' } };
  await vi.waitFor(() => expect(button('Refresh workspaces', true).disabled).toBe(false));
  button('Refresh workspaces', true).click();
  await vi.waitFor(() => expect(requests.filter((entry) => entry.operation === 'CodeWorkspaceInventory')).toHaveLength(4));
  await tick();
  expect(onmoved).not.toHaveBeenCalled();
  expect(localStorage.getItem(key)).not.toBeNull();

  answers.CodeWorkspaceReceipt = { managedWorkspaceRequest: { ...receipt, destinationDevice: destination, state: 'activated' } };
  button('Refresh workspaces', true).click();
  await vi.waitFor(() => expect(onmoved).toHaveBeenCalledExactlyOnceWith({ destination, workspace, generation: 7, previewPort: 4321 }));
  expect(localStorage.getItem(key)).toBeNull();
});

it('withdraws the retained move and unlocks the source only after acknowledgment', async () => {
  const move: Intent = {
    device: source, request, workspace,
    action: { kind: 'move', destination_device: destination, generation: 7, storage_bytes: quota },
  };
  const receipt = { transfer: request, workspace, sourceDevice: source, destinationDevice: destination };
  const answers: Record<string, unknown> = {
    CodeWorkspaceReceipt: { managedWorkspaceRequest: { ...receipt, state: 'expired' } },
    CodeWorkspaceSubmit: { queueManagedWorkspaceRequest: request },
    CodeWorkspaceInventory: { managedNodeWorkspaces: { device: source, available: true, workspaces: [] } },
  };
  const { hub, requests } = fakeHub(answers);
  const key = `anima-workspace-request:${hub.address.origin}:${source}`;
  localStorage.setItem(key, JSON.stringify(move));
  const onmoved = vi.fn();
  app = mount(Workspaces, { target: document.body, props: {
    api: nodeCodeApi(hub), device: source, origin: hub.address.origin, storage: localStorage,
    onuse: vi.fn(), onmoved,
  } });

  await vi.waitFor(() => expect(button('Withdraw move').disabled).toBe(false));
  button('Withdraw move').click();
  await tick();
  expect(localStorage.getItem(key)).not.toBeNull();
  expect(requests.some((entry) => entry.operation === 'CodeWorkspaceSubmit')).toBe(false);
  answers.CodeWorkspaceReceipt = { managedWorkspaceRequest: { ...receipt, state: 'withdrawing' } };
  button('Confirm withdrawal').click();
  await vi.waitFor(() => expect(document.body.textContent).toContain('Checking the destination'));
  expect(requests.find((entry) => entry.operation === 'CodeWorkspaceSubmit')?.variables)
    .toEqual({ device: source, input: { request, workspace, action: { kind: 'withdraw-move' } } });
  expect(localStorage.getItem(key)).not.toBeNull();

  await vi.waitFor(() => expect(button('Refresh workspaces', true).disabled).toBe(false));
  answers.CodeWorkspaceReceipt = { managedWorkspaceRequest: { ...receipt, state: 'withdrawn' } };
  button('Refresh workspaces', true).click();
  await vi.waitFor(() => expect(localStorage.getItem(key)).toBeNull());
  expect(onmoved).not.toHaveBeenCalled();
});

it('requires confirmation and sends the retained copy identity to the destination', async () => {
  const copy = { transfer: request, workspace, sourceDevice: source, sourceName: 'Builder', generation: 7, pending: false };
  const answers: Record<string, unknown> = {
    CodeWorkspaceSubmit: { queueManagedWorkspaceRequest: request },
    CodeWorkspaceInventory: {
      managedNodeWorkspaces: { device: destination, available: true, workspaces: [], retainedSources: [copy] },
    },
  };
  const { hub, requests } = fakeHub(answers);
  app = mount(Workspaces, { target: document.body, props: {
    api: nodeCodeApi(hub), device: destination, origin: hub.address.origin, storage: localStorage, onuse: vi.fn(),
  } });
  await vi.waitFor(() => expect(button('Delete retained copy on Builder', true).disabled).toBe(false));
  button('Delete retained copy on Builder', true).click();
  await tick();
  const mutations = () => requests.filter((entry) => entry.operation === 'CodeWorkspaceSubmit');
  expect(mutations()).toHaveLength(0);
  button('Cancel').click();
  await tick();
  expect(mutations()).toHaveLength(0);

  button('Delete retained copy on Builder', true).click();
  await tick();
  answers.CodeWorkspaceInventory = {
    managedNodeWorkspaces: { device: destination, available: true, workspaces: [], retainedSources: [{ ...copy, pending: true }] },
  };
  button('Delete retained copy').click();
  await vi.waitFor(() => expect(document.body.textContent).toContain('Waiting for cleanup on Builder'));
  expect(mutations()).toHaveLength(1);
  expect(mutations()[0]?.variables).toEqual({
    device: destination, input: { request, workspace, action: { kind: 'retire-source', confirmed: true } },
  });
});

it('queues an automatic move with no named destination when only the source is visible', async () => {
  const answers: Record<string, unknown> = { CodeWorkspaceInventory: { managedNodeWorkspaces: {
    device: source, available: true, workspaces: [{
      workspace, request, state: 'completed', action: { kind: 'reconcile', generation: 7 },
      receipt: { result: { workspace, generation: 7, phase: 'running', storageBytes: quota } },
    }],
  } } };
  const fake = fakeHub(answers);
  app = mount(Workspaces, { target: document.body, props: {
    api: nodeCodeApi(fake.hub), device: source, origin: fake.hub.address.origin, storage: localStorage, onuse: vi.fn(),
  } });
  await openMoveForm();
  selectDestination('automatic');
  await tick();
  button('Move workspace').click();
  const sent = () => fake.requests.find((entry) => entry.operation === 'CodeWorkspaceSubmit');
  await vi.waitFor(() => expect(sent()?.variables).toMatchObject({
    device: source, input: { workspace, action: { kind: 'move-auto', generation: 7, storage_bytes: quota } },
  }));
  expect(sent()?.variables).not.toHaveProperty('input.action.destination_device');
  expect(fake.requests.filter((entry) => entry.operation === 'CodeWorkspaceInventory')).toHaveLength(1);
  const key = `anima-workspace-request:${fake.hub.address.origin}:${source}`;
  const pending = JSON.parse(localStorage.getItem(key) ?? 'null') as Intent;
  answers.CodeWorkspaceReceipt = { managedWorkspaceRequest: {
    transfer: pending.request, workspace, sourceDevice: source, destinationDevice: null, state: 'waiting-placement',
  } };
  await vi.waitFor(() => expect(button('Refresh workspaces', true).disabled).toBe(false));
  button('Refresh workspaces', true).click();
  await vi.waitFor(() => expect(document.body.textContent).toContain('Waiting for a node with enough capacity.'));
  expect(button('Withdraw move').disabled).toBe(false);
});
