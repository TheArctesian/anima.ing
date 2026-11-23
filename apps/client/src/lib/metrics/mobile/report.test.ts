// @vitest-environment jsdom
import { expect, it, vi } from 'vitest';
import { createHub, parseHubAddress } from '$lib/hub/hub';
import { watchMobileMetrics } from './report';

const native = vi.hoisted(() => ({
  readLink: vi.fn(),
  enrol: vi.fn(),
  enrolled: vi.fn(),
  push: vi.fn(),
}));

vi.mock('$lib/desktop/link/shell', () => ({ readLink: native.readLink }));
vi.mock('$lib/desktop/metrics/mobile', () => ({
  enrolMobileReporter: native.enrol,
  hasMobileReporter: native.enrolled,
  pushMobileReadings: native.push,
}));

it('recovers reporting on the already enrolled device that owns the current link', async () => {
  native.readLink.mockResolvedValue({ ok: true, value: { joined: true, state: 'up', port: 47100, publicKey: 'nodekey:tablet' } });
  native.enrolled.mockResolvedValue(false);
  native.push.mockResolvedValueOnce(false).mockResolvedValue(true);

  const origin = 'http://127.0.0.1:47100';
  const address = parseHubAddress(origin);
  if (!address.ok) throw new Error('invalid test address');
  const api = createHub({ address: address.value, token: async () => null });
  const sent: { operation: string; variables: unknown }[] = [];
  const hub: typeof api = {
    ...api,
    request: async (document, variables) => {
      const definition = document.definitions[0];
      const operation = definition?.kind === 'OperationDefinition' ? definition.name?.value : undefined;
      let value;
      switch (operation) {
        case 'LinkDevices':
          value = { linkDevices: [{ key: 'nodekey:other', address: 'fd41::9' }, { key: 'nodekey:tablet', address: 'fd41::7' }] };
          break;
        case 'DeviceHosts':
          value = { devices: { edges: [
            { node: { id: 'desktop', host: 'fd41::9', metricsSource: 'NONE' } },
            { node: { id: 'tablet', host: 'fd41:0:0:0:0:0:0:7', metricsSource: 'NONE' } },
          ], pageInfo: { hasNextPage: false, endCursor: null } } };
          break;
        case 'EnrolDaemon':
          value = { enrolDaemon: { code: 'test-code' } };
          break;
        default:
          throw new Error(`Unexpected operation: ${operation ?? 'unknown'}`);
      }
      sent.push({ operation, variables });
      return { ok: true, value } as never;
    },
  };
  const stop = watchMobileMetrics(hub);
  try {
    await vi.waitFor(() => expect(native.enrol).toHaveBeenCalledExactlyOnceWith(origin, 'tablet', 'test-code'));
    expect(sent.filter(({ operation }) => operation === 'EnrolDaemon')).toEqual([{ operation: 'EnrolDaemon', variables: { deviceId: 'tablet' } }]);
    expect(native.push).toHaveBeenCalledTimes(2);
  } finally {
    stop();
    await api.close();
  }
});
