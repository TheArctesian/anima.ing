import { afterEach, describe, expect, it, vi } from 'vitest';
import type { CloudDevice } from '$lib/desktop/cloud/device';
import type { DaemonNeed } from '$lib/hub/generated/graphql';
import type { HubResult } from '$lib/hub/hub';
import type { HubDevice } from './here';
import { NEED_EVERY_MS, reportDaemonNeed, watchDaemonNeed, type DaemonNeedParts } from './need';

const MAC: CloudDevice = { name: 'stephen-s-macbook-pro', computer: "Stephen's MacBook Pro", os: 'MACOS' };
const ADDRESS = 'fd41:4e49:4d41::7';
const ok = <T>(value: T): HubResult<T> => ({ ok: true, value });

/** This Mac on the link, a device of the hub with `source`, whose mark is read from `waits`. */
function parts(waits: () => boolean, source: HubDevice['metricsSource'] = 'DAEMON') {
  return {
    device: vi.fn(() => Promise.resolve(MAC)),
    linkDevices: vi.fn(() => Promise.resolve(ok([{ name: MAC.name, address: ADDRESS }]))),
    devices: vi.fn(() => Promise.resolve(ok([{ id: 'mac', host: ADDRESS, metricsSource: source }]))),
    keychainWaits: vi.fn(() => Promise.resolve(waits())),
    report: vi.fn<DaemonNeedParts['report']>(() => Promise.resolve(ok(null))),
    log: vi.fn<(line: string) => void>(),
    found: vi.fn<(deviceId: string) => void>(),
  };
}

afterEach(() => {
  vi.useRealTimers();
});

describe('reportDaemonNeed (R-322)', () => {
  it('tells the hub when the mark appears and when it goes, for the device of this computer', async () => {
    const hub = parts(() => true);
    expect(await reportDaemonNeed(hub, null)).toBe('KEYCHAIN');
    expect(hub.report).toHaveBeenCalledExactlyOnceWith('mac', 'KEYCHAIN');
    expect(hub.found).toHaveBeenCalledWith('mac');
    hub.keychainWaits.mockResolvedValue(false);
    expect(await reportDaemonNeed(hub, 'KEYCHAIN')).toBeNull();
    expect(hub.report).toHaveBeenLastCalledWith('mac', null);
  });

  it('makes no call when nothing changed', async () => {
    const hub = parts(() => true);
    expect(await reportDaemonNeed(hub, 'KEYCHAIN')).toBe('KEYCHAIN');
    expect(hub.report).not.toHaveBeenCalled();
    expect(hub.devices).not.toHaveBeenCalled();
  });

  it.each<[string, Partial<ReturnType<typeof parts>>]>([
    ['a device that is not a daemon device', { devices: vi.fn(() => Promise.resolve(ok([{ id: 'mac', host: ADDRESS, metricsSource: 'NONE' as const }]))) }],
    ['a hub that refuses', { report: vi.fn(() => Promise.resolve<HubResult<unknown>>({ ok: false, error: { code: 'FORBIDDEN', message: 'x' } })) }],
  ])('keeps what it knew for %s, so that the next reading tries again', async (_, change) => {
    const hub = { ...parts(() => true), ...change };
    const before: DaemonNeed | null | undefined = undefined;
    expect(await reportDaemonNeed(hub, before)).toBeUndefined();
  });
});

describe('watchDaemonNeed (R-322)', () => {
  it('reads at once and each minute, and calls the hub only on a change', async () => {
    vi.useFakeTimers();
    let waits = false;
    const hub = parts(() => waits);
    const stop = watchDaemonNeed(hub);
    await vi.advanceTimersByTimeAsync(0);
    expect(hub.report.mock.calls).toEqual([['mac', null]]);
    await vi.advanceTimersByTimeAsync(NEED_EVERY_MS * 3);
    expect(hub.keychainWaits).toHaveBeenCalledTimes(4);
    expect(hub.report).toHaveBeenCalledTimes(1);
    waits = true;
    await vi.advanceTimersByTimeAsync(NEED_EVERY_MS);
    expect(hub.report).toHaveBeenLastCalledWith('mac', 'KEYCHAIN');
    stop();
    waits = false;
    await vi.advanceTimersByTimeAsync(NEED_EVERY_MS * 2);
    expect(hub.report).toHaveBeenCalledTimes(2);
  });
});
