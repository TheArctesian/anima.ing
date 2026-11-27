// @vitest-environment jsdom
import { mount, unmount } from 'svelte';
import { expect, it, vi } from 'vitest';
import { fakeHub } from '$lib/devices/hub.fixture';
import { endTransitionsAtOnce } from '$lib/devices/transitions.fixture';
import { nodeCodeApi } from '../api';
import { workspaces } from '../receipts';
import Previews from './Previews.svelte';

endTransitionsAtOnce();

it('prepares a detected port on the selected workspace only after the operator selects it', async () => {
  const device = '0190a000-0000-7000-8002-000000000002';
  const workspace = '0190a000-0000-7000-8002-000000000003';
  const [selected] = workspaces([{ workspace, generation: 7, enabled: true, ready: true,
    listeningPorts: [4321] }]) ?? [];
  if (!selected) throw new Error('Missing workspace fixture');
  const fake = fakeHub({ CodeNodePreviews: { managedCodePreviews: [] } });
  const app = mount(Previews, { target: document.body,
    props: { api: nodeCodeApi(fake.hub), device, workspace: selected, appOrigin: fake.hub.address.origin },
  });
  const requests = () => fake.requests.filter((request) => request.operation === 'CodeNodeOpenPreview');
  try {
    const button = await vi.waitFor(() => {
      const button = document.querySelector<HTMLButtonElement>('button[aria-label="Prepare preview on detected port 4321"]');
      if (!button) throw new Error('Detected port is not available in Code');
      return button;
    });
    expect(requests()).toHaveLength(0);
    button.click();
    await vi.waitFor(() => expect(requests()[0]?.variables).toMatchObject({
      device, input: { workspace, generation: '7', port: 4321 },
    }));
  } finally {
    await unmount(app);
    document.body.replaceChildren();
  }
});
