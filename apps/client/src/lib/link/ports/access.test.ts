import { expect, it, vi } from 'vitest';
import { fakeHub } from '$lib/devices/hub.fixture';
import { publishAccess } from './access';

const calls = vi.hoisted(() => ({ update: vi.fn().mockResolvedValue({ ok: true }) }));
vi.mock('$lib/desktop/link/shell', () => ({ readLink: () => ({ ok: true, value: { publicKey: 'local-key', port: 1234 } }) }));
vi.mock('$lib/link/devices', () => ({ deviceLinkAddress: () => ({ origin: 'https://hub.test' }) }));
vi.mock('$lib/link/api', () => ({ linkApi: () => ({ devices: () => ({ ok: true, value: [{ key: 'local-key', address: 'device.test' }] }) }) }));
vi.mock('$lib/devices/client', () => ({ devicesApi: () => ({
  deviceHosts: () => ({ ok: true, value: [{ host: 'device.test', id: 'device-id' }] }),
  get: () => ({ ok: true, value: {
    os: 'MACOS', ssh: { username: 'ssh-user', auth: 'PASSWORD' },
    vnc: { enabled: true, port: 5902, username: 'other-vnc-user' },
  } }),
  update: calls.update,
}) }));

it('publishes SSH alone without rewriting another account or port used by VNC', async () => {
  const { hub } = fakeHub();
  await publishAccess(hub, {
    configured: true, desktop: true, username: 'local-user',
    selected: [22, 5900], open: [22, 5900], ready: [22, 5900], listening: [], notes: [],
  }, 22);
  expect(calls.update).toHaveBeenCalledExactlyOnceWith('device-id', {
    ssh: { enabled: true, port: 22, username: 'ssh-user', auth: 'PASSWORD' },
  });
});
