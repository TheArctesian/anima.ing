import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { HubAddress } from '$lib/hub/hub';
import { signOutCommandLine, writeTokenAndCommandLine } from './sign-in';

const tauri = vi.hoisted(() => ({ invoke: vi.fn(), isTauri: vi.fn(() => true) }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: tauri.invoke, isTauri: tauri.isTauri }));

const ADDRESS: HubAddress = {
  origin: 'http://127.0.0.1:41641',
  http: 'http://127.0.0.1:41641/graphql',
  socket: 'ws://127.0.0.1:41641/graphql',
};

beforeEach(() => {
  tauri.invoke.mockReset();
  tauri.isTauri.mockReturnValue(true);
});

describe('writeTokenAndCommandLine (R-383)', () => {
  it('SGN-NEW-adopt: the app token is written first, and a failed write signs nothing in', async () => {
    const order: string[] = [];
    tauri.invoke.mockImplementation((command: string) => {
      order.push(command);
      return Promise.resolve(command === 'command_line_computer' ? 'studio' : undefined);
    });
    const write = vi.fn(() => {
      order.push('write');
      return Promise.resolve();
    });
    await writeTokenAndCommandLine(write)(ADDRESS, 'app-token');
    expect(order).toEqual(['write', 'command_line_adopt']);
    expect(tauri.invoke).toHaveBeenCalledWith('command_line_adopt', { hub: ADDRESS.origin, token: 'app-token' });

    order.length = 0;
    const refused = writeTokenAndCommandLine(() => Promise.reject(new Error('key store')));
    await expect(refused(ADDRESS, 'app-token')).rejects.toThrow('key store');
    expect(order).toEqual([]);
  });

  it('SGN-NEW-adopt: off desktop nothing is asked of a shell', async () => {
    tauri.isTauri.mockReturnValue(false);
    await writeTokenAndCommandLine(() => Promise.resolve())(ADDRESS, 'app-token');
    expect(tauri.invoke).not.toHaveBeenCalled();
    expect(await signOutCommandLine(ADDRESS)).toBe(true);
  });
});

describe('signOutCommandLine (R-383)', () => {
  it('SGN-NEW-adopt: anima signs out of the same hub, and a shell that fails is told', async () => {
    tauri.invoke.mockResolvedValueOnce(undefined);
    expect(await signOutCommandLine(ADDRESS)).toBe(true);
    expect(tauri.invoke).toHaveBeenCalledWith('command_line_sign_out', { hub: ADDRESS.origin });
    tauri.invoke.mockRejectedValueOnce({ code: 'refused' });
    expect(await signOutCommandLine(ADDRESS)).toBe(false);
  });
});
