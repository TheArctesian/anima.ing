// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import OpenPortRows from './OpenPortRows.svelte';

const mocks = vi.hoisted(() => ({ linkPorts: vi.fn() }));
vi.mock('$lib/desktop/link/shell', async (original) => ({
  ...(await original<typeof import('$lib/desktop/link/shell')>()),
  linkPorts: mocks.linkPorts,
}));

let app: ReturnType<typeof mount> | undefined;

async function settle(): Promise<void> {
  for (let round = 0; round < 3; round += 1) {
    await new Promise((resolve) => setTimeout(resolve, 0));
    flushSync();
  }
}

async function shown(): Promise<HTMLElement> {
  const target = document.createElement('div');
  document.body.append(target);
  app = mount(OpenPortRows, { target });
  await settle();
  return target;
}

const switches = (target: HTMLElement) => [...target.querySelectorAll<HTMLButtonElement>('[role="switch"]')];

beforeEach(() => {
  vi.stubGlobal('navigator', { userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)' });
  mocks.linkPorts.mockReset().mockImplementation((change?: { port: number; on: boolean }) =>
    Promise.resolve({ ok: true, value: { open: change?.on === true ? [change.port] : [] } }),
  );
});

afterEach(async () => {
  if (app !== undefined) await unmount(app);
  app = undefined;
  document.body.innerHTML = '';
  vi.unstubAllGlobals();
});

it('HMD-049: each port is off until the person turns it on, and the row names the setting of the system', async () => {
  const target = await shown();
  expect(mocks.linkPorts).toHaveBeenCalledExactlyOnceWith();
  expect(switches(target).map((button) => button.getAttribute('aria-checked'))).toEqual(['false', 'false']);
  expect(target.textContent).toContain('administrator approval');
  expect(target.textContent).toContain('Screen Sharing');
});

it('HMD-049: one click turns a port on, and the switch shows the list of the computer', async () => {
  const target = await shown();
  switches(target)[1]?.click();
  await settle();
  expect(mocks.linkPorts).toHaveBeenLastCalledWith({ port: 5900, on: true });
  expect(switches(target).map((button) => button.getAttribute('aria-checked'))).toEqual(['false', 'true']);
});

it('a change that the link program refuses leaves the switch as the list is, and says why', async () => {
  const target = await shown();
  mocks.linkPorts.mockResolvedValueOnce({ ok: false, problem: 'failed', message: 'The link did not finish. See the application log.' });
  switches(target)[0]?.click();
  await settle();
  expect(switches(target)[0]?.getAttribute('aria-checked')).toBe('false');
  expect(target.querySelector('[role="alert"]')?.textContent).toContain('Could not turn the port on.');
});

it('a computer that has not joined a hub shows no switch', async () => {
  mocks.linkPorts.mockReset().mockResolvedValue({ ok: false, problem: 'folder', message: 'x' });
  const target = await shown();
  expect(switches(target)).toHaveLength(0);
});

it('SSH setup waits for system approval, prevents duplicate requests and restores off after cancellation', async () => {
  const target = await shown();
  let finish!: (value: unknown) => void;
  mocks.linkPorts.mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  switches(target)[0]?.click();
  await settle();
  expect(target.querySelector('[role="status"]')?.textContent).toContain('authorization prompt');
  expect(switches(target).every((button) => button.getAttribute('aria-disabled') === 'true')).toBe(true);
  switches(target)[0]?.click();
  expect(mocks.linkPorts).toHaveBeenCalledTimes(2);
  finish({ ok: false, problem: 'ssh-setup', message: 'Administrator authorization was cancelled.' });
  await settle();
  expect(switches(target)[0]?.getAttribute('aria-checked')).toBe('false');
  expect(switches(target).every((button) => button.getAttribute('aria-disabled') !== 'true')).toBe(true);
  expect(target.querySelector('[role="status"]')).toBeNull();
  expect(target.querySelector('[role="alert"]')?.textContent).toContain('cancelled');
});
