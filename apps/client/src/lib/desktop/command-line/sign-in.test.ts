import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { HubAddress } from '$lib/hub/hub';
import { signInCommandLine, signOutCommandLine, UNNAMED_COMPUTER, writeTokenAndCommandLine, type CommandLineSignIn } from './sign-in';

const tauri = vi.hoisted(() => ({ invoke: vi.fn(), isTauri: vi.fn(() => true) }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: tauri.invoke, isTauri: tauri.isTauri }));

const ADDRESS: HubAddress = {
  origin: 'http://127.0.0.1:41641',
  http: 'http://127.0.0.1:41641/graphql',
  socket: 'ws://127.0.0.1:41641/graphql',
};

function parts(overrides: Partial<CommandLineSignIn> = {}): CommandLineSignIn & { adopted: [string, string][] } {
  const adopted: [string, string][] = [];
  return {
    adopted,
    computer: () => Promise.resolve('studio'),
    issue: () => Promise.resolve({ ok: true, value: 'command-line-token' }),
    adopt: (origin, token) => {
      adopted.push([origin, token]);
      return Promise.resolve();
    },
    ...overrides,
  };
}

beforeEach(() => {
  tauri.invoke.mockReset();
  tauri.isTauri.mockReturnValue(true);
});

describe('signInCommandLine (R-383, SGN-NEW-adopt)', () => {
  it('SGN-NEW-adopt: the token that the hub links to the app token goes to anima for the same hub', async () => {
    const issue = vi.fn<CommandLineSignIn['issue']>(() => Promise.resolve({ ok: true, value: 'command-line-token' }));
    const signIn = parts({ issue });
    expect(await signInCommandLine(signIn, ADDRESS, 'app-token')).toBeNull();
    expect(issue).toHaveBeenCalledWith(ADDRESS, 'app-token', 'studio');
    expect(signIn.adopted).toEqual([[ADDRESS.origin, 'command-line-token']]);
  });

  it('SGN-NEW-adopt: a computer with no name is named, and a refusal or a failure adopts nothing', async () => {
    const issue = vi.fn<CommandLineSignIn['issue']>(() => Promise.resolve({ ok: false, error: { code: 'FORBIDDEN', message: 'x' } }));
    const refused = parts({ computer: () => Promise.resolve('  '), issue });
    expect(await signInCommandLine(refused, ADDRESS, 'app-token')).toBe('refused');
    expect(issue).toHaveBeenCalledWith(ADDRESS, 'app-token', UNNAMED_COMPUTER);
    expect(refused.adopted).toEqual([]);

    const broken = parts({ adopt: () => Promise.reject(new Error('the shell did not answer')) });
    expect(await signInCommandLine(broken, ADDRESS, 'app-token')).toBe('failed');
  });
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
    expect(order[0]).toBe('write');

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
