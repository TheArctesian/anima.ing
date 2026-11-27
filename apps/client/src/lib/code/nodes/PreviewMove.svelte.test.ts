// @vitest-environment jsdom
import { mount, unmount } from 'svelte';
import { expect, it, vi } from 'vitest';
import { fakeHub } from '$lib/devices/hub.fixture';
import { endTransitionsAtOnce } from '$lib/devices/transitions.fixture';
import type { Hub } from '$lib/hub/hub';
import NodeCode from './NodeCode.svelte';

endTransitionsAtOnce();

const current = vi.hoisted(() => ({ hub: undefined as Hub | undefined }));
vi.mock('$lib/hub/context', () => ({ useHub: () => current.hub }));

const source = '0190a000-0000-7000-8002-000000000001';
const destination = '0190a000-0000-7000-8002-000000000002';
const workspace = '0190a000-0000-7000-8002-000000000003';
const transfer = '0190a000-0000-7000-8002-000000000004';
const other = '0190a000-0000-7000-8002-000000000005';
const port = 4321;

function workspaceRow(workspace: string, generation: number, listeningPorts: number[]) {
  return { workspace, generation, enabled: true, ready: true, listeningPorts };
}

function byLabel(label: string): HTMLButtonElement {
  const element = [...document.querySelectorAll('button')].find((button) =>
    (button.getAttribute('aria-label') ?? button.textContent?.trim()) === label);
  if (!element) throw new Error(`Missing action: ${label}`);
  return element;
}

function setup() {
  const otherRow = workspaceRow(other, 8, [port]);
  const answers: Record<string, unknown> = {
    DeviceHosts: { devices: { edges: [destination, source].map((id) => ({ node: { id, name: id === source ? 'Source' : 'Destination' } })),
      pageInfo: { hasNextPage: false } } },
    CodeNodeWorkspaces: { managedCodeWorkspaces: [otherRow, workspaceRow(workspace, 7, [port])] },
    CodeNodePreviews: { managedCodePreviews: [] },
    CodeWorkspaceReceipt: { managedWorkspaceRequest: {
      transfer, workspace, sourceDevice: source, destinationDevice: destination, state: 'activated',
    } },
  };
  const fake = fakeHub(answers);
  const openings = () => fake.requests.filter((request) => request.operation === 'CodeNodeOpenPreview');
  const links = () => fake.requests.filter((request) => request.operation === 'CodeNodePreviewHandoff');
  const endpoint = () => (openings().at(-1)?.variables.input as { endpoint: string } | undefined)?.endpoint;
  let issuedGeneration = 7;
  Object.defineProperties(answers, {
    CodeNodeOpenPreview: { get: () => ({ openManagedCodePreview: {
      endpoint: endpoint(), device: destination, workspace, port, generation: issuedGeneration,
      expiresAt: new Date(Date.now() + 60_000).toISOString(), revoked: false,
    } }) },
    CodeNodePreviewHandoff: { get: () => ({ managedCodePreviewHandoff: {
      action: `https://${endpoint()?.replaceAll('-', '') ?? ''}.preview.test/.anima/handoff`,
      method: 'POST', token: 'e'.repeat(43),
    } }) },
  });
  current.hub = fake.hub;
  localStorage.setItem(`anima-workspace-request:${fake.hub.address.origin}:${source}`, JSON.stringify({
    device: source, request: transfer, workspace, previewPort: port,
    action: { kind: 'move-auto', generation: 7, storage_bytes: 2 ** 30 },
  }));
  return {
    openings, links, endpoint,
    ready: (ports: number[]) => {
      answers.CodeNodeWorkspaces = { managedCodeWorkspaces: [otherRow, workspaceRow(workspace, 8, ports)] };
      byLabel('Refresh nodes').click();
    },
    verifyDestination: () => { issuedGeneration = 8; },
  };
}

it('recovers the selected port and requests a fresh handoff only for the moved workspace generation', async () => {
  const fixture = setup();
  const app = mount(NodeCode, { target: document.body });
  const reconnect = () => byLabel(`Reconnect preview on port ${port}`);
  const open = () => byLabel(`Open preview on port ${port}`);
  const popup = { close: vi.fn() } as unknown as Window;
  const windowOpen = vi.spyOn(window, 'open').mockReturnValue(popup);
  try {
    await vi.waitFor(() => expect(document.body.textContent).toContain(`Preview :${port} is waiting for the workspace.`));
    expect(document.querySelector('[aria-label="Workspace previews"]')).toBeNull();
    expect(fixture.openings()).toHaveLength(0);
    fixture.ready([]);
    await vi.waitFor(() => expect(reconnect().disabled).toBe(true));
    fixture.ready([port]);
    await vi.waitFor(() => expect(reconnect().disabled).toBe(false));
    expect(fixture.openings()).toHaveLength(0);

    reconnect().click();
    await vi.waitFor(() => expect(document.body.textContent).toContain('The hub returned a different or inactive preview.'));
    const first = fixture.endpoint();
    expect(first).toBeDefined();
    expect(fixture.openings()[0]?.variables).toEqual({ device: destination, input: { endpoint: first, workspace, generation: '8', port } });
    expect(fixture.links()).toHaveLength(0);
    expect(windowOpen).not.toHaveBeenCalled();

    fixture.verifyDestination();
    reconnect().click();
    await vi.waitFor(() => expect(open().disabled).toBe(false));
    expect(fixture.links().map((row) => row.variables)).toEqual([{ endpoint: first }]);
    expect(windowOpen).not.toHaveBeenCalled();
    open().click();
    expect(windowOpen).toHaveBeenCalledExactlyOnceWith(`https://${first?.replaceAll('-', '') ?? ''}.preview.test/.anima/connect`, '_blank');
  } finally {
    await unmount(app);
    windowOpen.mockRestore();
    localStorage.clear();
    document.body.replaceChildren();
  }
  expect(popup.close).toHaveBeenCalledExactlyOnceWith();
});
