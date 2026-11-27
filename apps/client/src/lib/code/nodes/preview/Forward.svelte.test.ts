// @vitest-environment jsdom
import { mount, unmount } from 'svelte';
import { expect, it, vi } from 'vitest';
import { fakeHub } from '$lib/devices/hub.fixture';
import { nodeCodeApi } from '../api';
import Forward from './Forward.svelte';

const native = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: native.invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: native.listen }));

it('keeps a port conflict visible and stops the exact listener when Code closes', async () => {
  const endpoint = '0192f3a4-5b6c-7d8e-9fa0-b1c2d3e4f5a6';
  const action = 'https://0192f3a45b6c7d8e9fa0b1c2d3e4f5a6.preview.example/.anima/handoff';
  const fake = fakeHub({ CodeNodePreviewHandoff: {
    managedCodePreviewHandoff: { action, token: 'A'.repeat(43), method: 'POST' },
  } });
  native.listen.mockResolvedValue(() => {});
  native.invoke.mockResolvedValue(undefined).mockRejectedValueOnce('port_in_use').mockResolvedValueOnce('http://127.0.0.1:3001');
  const app = mount(Forward, { target: document.body, props: {
    api: nodeCodeApi(fake.hub), endpoint, port: 3000, appOrigin: fake.hub.address.origin, onclose: vi.fn(),
  } });
  let operation: string | undefined;
  try {
    const start = document.querySelector<HTMLButtonElement>('button[type=submit]')!;
    await vi.waitFor(() => expect(start.disabled).toBe(false));
    expect(native.invoke).not.toHaveBeenCalled();
    start.click();
    await vi.waitFor(() => expect(document.querySelector('[role=alert]')?.textContent).toContain('local port is in use'));
    const input = document.querySelector('input')!;
    input.value = '3001';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    start.click();
    await vi.waitFor(() => expect(document.querySelector('code')?.textContent).toBe('http://127.0.0.1:3001'));
    const opening = native.invoke.mock.calls[1]?.[1].opening;
    operation = opening.id;
    expect(opening).toMatchObject({ endpoint, localPort: 3001 });
    expect(fake.requests.filter((request) => request.operation === 'CodeNodePreviewHandoff')).toHaveLength(1);
  } finally {
    await unmount(app);
    document.body.replaceChildren();
  }
  expect(native.invoke).toHaveBeenLastCalledWith('workspace_forward_stop', { id: operation });
});
