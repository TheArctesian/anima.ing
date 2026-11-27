import { expect, it, vi } from 'vitest';
import type { OsUpdateCalls } from './api';
import { OsUpdate } from './update.svelte';

it('OS5-R1: remote restart sends the release that was reviewed despite a status refresh', async () => {
  const reviewed = 'a'.repeat(64);
  const newer = 'b'.repeat(64);
  const calls: OsUpdateCalls = {
    status: vi.fn<OsUpdateCalls['status']>()
      .mockResolvedValueOnce({ ok: true, value: { deviceOsUpdate: {
        phase: 'staged', version: '20300101000000', review: { activate: reviewed },
      } } })
      .mockResolvedValueOnce({ ok: true, value: { deviceOsUpdate: {
        phase: 'staged', version: '20300102000000', review: { activate: newer },
      } } })
      .mockResolvedValue({ ok: false, error: { code: 'UNAVAILABLE', message: 'Device restarting' } }),
    request: vi.fn().mockResolvedValue({ ok: true, value: { requestDeviceOsUpdate: true } }),
  };
  const update = new OsUpdate(calls, 'test-device');
  await update.refresh();
  await update.request('ACTIVATE');
  expect(calls.request).not.toHaveBeenCalled();
  await update.refresh();
  await update.request('ACTIVATE', true);
  expect(calls.request).toHaveBeenCalledWith('test-device', 'ACTIVATE', true, reviewed);
  expect(update.status).toBeNull();
  expect(update.error).toBe('Device restarting');
});

it('OS5-R1: a disconnected device hides stale ready-to-restart controls', async () => {
  const status = vi.fn<OsUpdateCalls['status']>()
    .mockResolvedValueOnce({ ok: true, value: { deviceOsUpdate: { phase: 'staged', version: '20300101000000' } } })
    .mockResolvedValueOnce({ ok: false, error: { code: 'UNAVAILABLE', message: 'Device offline' } });
  const update = new OsUpdate({ status, request: vi.fn() }, 'test-device');
  await update.refresh();
  expect(update.status?.phase).toBe('staged');
  await update.refresh();
  expect(update.status).toBeNull();
  expect(update.error).toBe('Device offline');
});
