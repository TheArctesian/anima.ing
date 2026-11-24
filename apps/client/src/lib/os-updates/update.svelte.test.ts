import { expect, it, vi } from 'vitest';
import type { OsUpdateCalls } from './api';
import { OsUpdate } from './update.svelte';

it('OS5-R1: restart waits for confirmation and an unavailable device does not appear updated', async () => {
  const calls: OsUpdateCalls = {
    status: vi.fn().mockResolvedValue({ ok: false, error: { code: 'UNAVAILABLE', message: 'Device restarting' } }),
    request: vi.fn().mockResolvedValue({ ok: true, value: { requestDeviceOsUpdate: true } }),
  };
  const update = new OsUpdate(calls, 'test-device');
  await update.request('ACTIVATE');
  expect(update.confirmation).toBe('ACTIVATE');
  expect(calls.request).not.toHaveBeenCalled();
  await update.request('ACTIVATE', true);
  expect(calls.request).toHaveBeenCalledWith('test-device', 'ACTIVATE', true);
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
