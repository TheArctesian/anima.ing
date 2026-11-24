import { afterEach, describe, expect, it, vi } from 'vitest';
import type { CloudDevice } from '$lib/desktop/cloud/device';
import type { DaemonNeed } from '$lib/hub/generated/graphql';
import type { HubResult } from '$lib/hub/hub';
import type { HubDevice } from './here';
import { NEED_EVERY_MS, NEED_TURN_MS, reportDaemonNeed, watchDaemonNeed, type DaemonNeedParts } from './need';
import type { KeychainMark } from './shell';

const MAC: CloudDevice = { name: 'stephen-s-macbook-pro', computer: "Stephen's MacBook Pro", os: 'MACOS' };
const ADDRESS = 'fd41:4e49:4d41::7';
const ok = <T>(value: T): HubResult<T> => ({ ok: true, value });
const refused = (code: 'NOT_FOUND' | 'FORBIDDEN'): HubResult<DaemonNeed | null> => ({ ok: false, error: { code, message: 'x' } });
const UNREACHABLE: HubResult<DaemonNeed | null> = { ok: false, error: { code: 'unreachable', message: 'x' } };

function parts(mark: () => KeychainMark, source: HubDevice['metricsSource'] = 'DAEMON') {
  return {
    device: vi.fn(() => Promise.resolve(MAC)),
    linkDevices: vi.fn(() => Promise.resolve(ok([{ name: MAC.name, address: ADDRESS }]))),
    devices: vi.fn(() => Promise.resolve(ok([{ id: 'mac', host: ADDRESS, metricsSource: source }]))),
    keychainMark: vi.fn(() => Promise.resolve(mark())),
    report: vi.fn<DaemonNeedParts['report']>((_, need) => Promise.resolve(ok(need))),
    signedIn: vi.fn(() => true),
    log: vi.fn<(line: string) => void>(),
    found: vi.fn<(deviceId: string) => void>(),
  };
}

afterEach(() => {
  vi.useRealTimers();
});

describe('reportDaemonNeed (R-322)', () => {
  it('tells the hub when the mark appears and when it goes, and keeps the device it found', async () => {
    const hub = parts(() => 'waits');
    const first = await reportDaemonNeed(hub, { reported: null });
    expect(first).toEqual({ reported: 'KEYCHAIN', deviceId: 'mac' });
    expect(hub.report).toHaveBeenCalledExactlyOnceWith('mac', 'KEYCHAIN');
    expect(hub.found).toHaveBeenCalledWith('mac');
    hub.keychainMark.mockResolvedValue('free');
    expect(await reportDaemonNeed(hub, first)).toEqual({ reported: null, deviceId: 'mac' });
    expect(hub.report).toHaveBeenLastCalledWith('mac', null);
    expect(hub.devices).toHaveBeenCalledTimes(1);
  });

  it.each<[string, Partial<ReturnType<typeof parts>>]>([
    ['nothing changed', {}],
    ['the mark was not read', { keychainMark: vi.fn(() => Promise.resolve<KeychainMark>('unknown')) }],
    ['the person is signed out', { keychainMark: vi.fn(() => Promise.resolve<KeychainMark>('free')), signedIn: vi.fn(() => false) }],
  ])('makes no call when %s', async (_, change) => {
    const hub = { ...parts(() => 'waits'), ...change };
    expect(await reportDaemonNeed(hub, { reported: 'KEYCHAIN' })).toEqual({ reported: 'KEYCHAIN' });
    expect(hub.report).not.toHaveBeenCalled();
    expect(hub.devices).not.toHaveBeenCalled();
  });

  it.each<[string, Partial<ReturnType<typeof parts>>]>([
    ['a device that is not a daemon device', { devices: vi.fn(() => Promise.resolve(ok([{ id: 'mac', host: ADDRESS, metricsSource: 'NONE' as const }]))) }],
    ['a computer that is not a device', { devices: vi.fn(() => Promise.resolve(ok<HubDevice[]>([]))) }],
  ])('settles the need for %s, with no call to set it', async (_, change) => {
    const hub = { ...parts(() => 'waits'), ...change };
    expect(await reportDaemonNeed(hub, { reported: undefined })).toEqual({ reported: 'KEYCHAIN' });
    expect(hub.report).not.toHaveBeenCalled();
  });

  it.each(['NOT_FOUND', 'FORBIDDEN'] as const)('settles the need when the hub answers %s, so that the next call waits for a change', async (code) => {
    const hub = parts(() => 'waits');
    hub.report.mockResolvedValue(refused(code));
    expect(await reportDaemonNeed(hub, { reported: undefined })).toEqual({ reported: 'KEYCHAIN', deviceId: 'mac' });
  });

  it('keeps what it knew when the hub kept no need, as a batch came just before, so that the next reading tries again', async () => {
    const hub = parts(() => 'waits');
    hub.report.mockResolvedValueOnce(ok(null));
    expect(await reportDaemonNeed(hub, { reported: null })).toEqual({ reported: null, deviceId: 'mac' });
  });

  it('keeps what it knew when the hub does not answer, so that the next reading tries again', async () => {
    const hub = parts(() => 'waits');
    hub.report.mockResolvedValue(UNREACHABLE);
    expect(await reportDaemonNeed(hub, { reported: undefined })).toEqual({ reported: undefined, deviceId: 'mac' });
  });
});

describe('watchDaemonNeed (R-322)', () => {
  it('reads at once and each minute, and calls the hub only on a change', async () => {
    vi.useFakeTimers();
    let mark: KeychainMark = 'free';
    const hub = parts(() => mark);
    const stop = watchDaemonNeed(hub);
    await vi.advanceTimersByTimeAsync(0);
    expect(hub.report.mock.calls).toEqual([['mac', null]]);
    await vi.advanceTimersByTimeAsync(NEED_EVERY_MS * 3);
    expect(hub.keychainMark).toHaveBeenCalledTimes(4);
    expect(hub.report).toHaveBeenCalledTimes(1);
    mark = 'unknown';
    await vi.advanceTimersByTimeAsync(NEED_EVERY_MS);
    expect(hub.report).toHaveBeenCalledTimes(1);
    mark = 'waits';
    await vi.advanceTimersByTimeAsync(NEED_EVERY_MS);
    expect(hub.report).toHaveBeenLastCalledWith('mac', 'KEYCHAIN');
    stop();
    mark = 'free';
    await vi.advanceTimersByTimeAsync(NEED_EVERY_MS * 2);
    expect(hub.report).toHaveBeenCalledTimes(2);
  });

  it('asks a hub that refused no more until the mark changes', async () => {
    vi.useFakeTimers();
    const hub = parts(() => 'waits');
    hub.report.mockResolvedValue(refused('NOT_FOUND'));
    const stop = watchDaemonNeed(hub);
    await vi.advanceTimersByTimeAsync(NEED_EVERY_MS * 5);
    expect(hub.report).toHaveBeenCalledTimes(1);
    expect(hub.log).toHaveBeenCalledTimes(1);
    stop();
  });

  it('ends a turn that does not answer, and the next turn runs', async () => {
    vi.useFakeTimers();
    const hub = parts(() => 'waits');
    hub.report.mockReturnValueOnce(new Promise(() => undefined));
    const stop = watchDaemonNeed(hub);
    await vi.advanceTimersByTimeAsync(NEED_TURN_MS);
    await vi.advanceTimersByTimeAsync(NEED_EVERY_MS);
    expect(hub.report).toHaveBeenCalledTimes(2);
    stop();
  });
});
