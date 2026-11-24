// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { installAnimations } from '$lib/forms/animations.fixture';
import AccessSetup from './AccessSetup.svelte';

const calls = vi.hoisted(() => ({ localAccess: vi.fn(), linkPorts: vi.fn(), publishAccess: vi.fn(), publishedAccess: vi.fn(), checkAccessHub: vi.fn(), settings: vi.fn() }));
vi.mock('$lib/desktop/link/access', () => ({ localAccess: calls.localAccess }));
vi.mock('$lib/desktop/link/shell', () => ({ linkPorts: calls.linkPorts }));
vi.mock('./access', () => ({ publishAccess: calls.publishAccess, publishedAccess: calls.publishedAccess, checkAccessHub: calls.checkAccessHub }));
vi.mock('$lib/desktop/system-settings', () => ({ openSharingSettings: calls.settings }));
vi.mock('$lib/hub/context', () => ({ useHub: () => ({}) }));
vi.mock('motion', async (original) => ({ ...(await original<typeof import('motion')>()), animate: (await import('$lib/motion/engine.fixture')).fakeRun }));

const fresh: import('$lib/desktop/link/access').Access = { configured: false, desktop: true, username: 'operator', selected: [22, 5900], open: [], ready: [], listening: [], notes: [] };
let app: ReturnType<typeof mount> | undefined;

async function settle(): Promise<void> {
  for (let i = 0; i < 3; i += 1) { await new Promise((done) => setTimeout(done, 0)); flushSync(); }
}

async function show(): Promise<void> {
  const target = document.createElement('div');
  document.body.append(target);
  app = mount(AccessSetup, { target, props: { automatic: true } });
  await settle();
}

beforeEach(() => {
  installAnimations();
  calls.publishedAccess.mockReset().mockResolvedValue([]);
  calls.linkPorts.mockReset().mockResolvedValue({ ok: true, value: { open: [] } });
  calls.settings.mockReset().mockResolvedValue(undefined);
  calls.checkAccessHub.mockReset().mockResolvedValue('local-key');
  vi.stubGlobal('navigator', { userAgent: 'Macintosh' });
  calls.localAccess.mockReset().mockResolvedValue(fresh);
  calls.publishAccess.mockReset().mockResolvedValue(undefined);
});

afterEach(async () => {
  if (app) await unmount(app);
  app = undefined;
  document.body.innerHTML = '';
  vi.unstubAllGlobals();
});

it('offers macOS RDP unselected by default and submits the selected connections', async () => {
  calls.localAccess.mockResolvedValue({ ...fresh, macRdp: true });
  await show();
  const switches = [...document.querySelectorAll<HTMLButtonElement>('[role="switch"]')];
  expect(switches.map((button) => button.getAttribute('aria-checked'))).toEqual(['true', 'false', 'true']);
  expect(calls.localAccess).toHaveBeenCalledExactlyOnceWith();
  switches[2]?.click();
  await settle();
  calls.localAccess.mockResolvedValueOnce({ ...fresh, configured: true, selected: [22], open: [22], ready: [22] });
  [...document.querySelectorAll('button')].find((button) => button.textContent.trim() === 'Continue')?.click();
  await settle();
  expect(calls.localAccess).toHaveBeenLastCalledWith([22], undefined);
  expect(calls.publishAccess).toHaveBeenCalledOnce();
});

it('never offers defaults again over a saved all-off choice', async () => {
  calls.localAccess.mockResolvedValue({ ...fresh, configured: true, selected: [] });
  await show();
  expect(document.querySelector('[role="switch"]')).toBeNull();
  expect(calls.publishAccess).not.toHaveBeenCalled();
});

it('headless setup keeps desktop protocols off and unavailable', async () => {
  calls.localAccess.mockResolvedValue({ ...fresh, desktop: false, selected: [22] });
  await show();
  const switches = [...document.querySelectorAll<HTMLButtonElement>('[role="switch"]')];
  expect(switches[1]?.getAttribute('aria-checked')).toBe('false');
  expect(switches[1]?.getAttribute('aria-disabled')).toBe('true');
  expect(document.body.textContent).toContain('No graphical desktop detected');
});

it('refuses setup before changing this computer when the selected hub no longer matches', async () => {
  await show();
  calls.checkAccessHub.mockRejectedValueOnce(new Error('Connect this computer to this hub.'));
  [...document.querySelectorAll('button')].find((button) => button.textContent.trim() === 'Continue')?.click();
  await settle();
  expect(calls.localAccess).toHaveBeenCalledExactlyOnceWith();
  expect(calls.publishAccess).not.toHaveBeenCalled();
  expect(document.body.textContent).toContain('Connect this computer to this hub.');
});


it('enables only the requested capability and reports progress until verified', async () => {
  await show();
  let finish: (value: typeof fresh) => void = () => undefined;
  calls.localAccess.mockImplementationOnce(() => new Promise<typeof fresh>((resolve) => { finish = resolve; }));
  const enable = document.querySelector<HTMLButtonElement>('[aria-label="Enable SSH"]');
  enable?.focus();
  enable?.click();
  await settle();
  expect(calls.localAccess).toHaveBeenLastCalledWith([22, 5900], 22);
  expect(document.body.textContent).toContain('Preparing…');
  expect(document.body.textContent).not.toContain('Ready in Anima');
  expect(document.activeElement).toBe(enable);
  enable?.click();
  expect(calls.localAccess).toHaveBeenCalledTimes(2);
  finish({ ...fresh, configured: true, open: [22], ready: [22] });
  await settle();
  expect(document.body.textContent).toContain('Ready in Anima');
  expect(calls.publishAccess).toHaveBeenCalledOnce();
  expect(calls.publishAccess.mock.calls[0]?.[2]).toBe(22);
  expect(document.activeElement).toBe(enable);
});

it('keeps a failed capability actionable and does not call it ready', async () => {
  await show();
  calls.localAccess.mockResolvedValueOnce({ ...fresh, notes: [{ port: 22, message: 'Administrator authorization was cancelled.' }] });
  document.querySelector<HTMLButtonElement>('[aria-label="Enable SSH"]')?.click();
  await settle();
  expect(document.body.textContent).toContain('Administrator authorization was cancelled.');
  expect(document.body.textContent).not.toContain('Ready in Anima');
  expect(document.querySelector('[aria-label="Enable SSH"]')).not.toBeNull();
});

it('opens the required macOS setting and verifies just screen sharing on return', async () => {
  await show();
  [...document.querySelectorAll('button')].find((button) => button.textContent.trim() === 'Open settings…')?.click();
  await settle();
  expect(calls.settings).toHaveBeenCalledOnce();
  expect(calls.localAccess).toHaveBeenCalledTimes(1);
  expect(document.body.textContent).not.toContain('Ready in Anima');
  calls.localAccess.mockResolvedValueOnce({ ...fresh, configured: true, open: [5900], ready: [5900] });
  window.dispatchEvent(new Event('focus'));
  await settle();
  expect(calls.localAccess).toHaveBeenLastCalledWith([22, 5900], 5900);
  expect(document.body.textContent).toContain('Ready in Anima');
});


it('honors a screen-sharing opt-out while System Settings is open', async () => {
  await show();
  [...document.querySelectorAll('button')].find((button) => button.textContent.trim() === 'Open settings…')?.click();
  await settle();
  document.querySelector<HTMLButtonElement>('#setup-port-5900')?.click();
  window.dispatchEvent(new Event('focus'));
  await settle();
  expect(calls.localAccess).toHaveBeenCalledExactlyOnceWith();
  expect(calls.publishAccess).not.toHaveBeenCalled();
});


it('retries failed publication without preparing protocols and never calls unpublished access ready', async () => {
  await show();
  const ready = { ...fresh, configured: true, open: [22], ready: [22] };
  calls.localAccess.mockResolvedValue(ready);
  calls.publishAccess.mockRejectedValueOnce(new Error('Hub update failed. Retry.'));
  document.querySelector<HTMLButtonElement>('[aria-label="Enable SSH"]')?.click();
  await settle();
  expect(document.body.textContent).toContain('Hub update failed. Retry.');
  expect(document.body.textContent).not.toContain('Ready in Anima');
  document.querySelector<HTMLButtonElement>('#setup-port-5900')?.click();
  await settle();
  const sync = document.querySelector<HTMLButtonElement>('[aria-label="Enable in Anima: SSH"]');
  expect(sync?.getAttribute('aria-disabled')).toBe('false');
  sync?.click();
  await settle();
  expect(calls.localAccess).toHaveBeenLastCalledWith();
  expect(calls.localAccess.mock.calls.filter((args) => args.length > 0)).toHaveLength(1);
  expect(calls.publishAccess).toHaveBeenCalledTimes(2);
  expect(document.body.textContent).toContain('Ready in Anima');
  expect(document.querySelector('#setup-port-5900')?.getAttribute('aria-checked')).toBe('false');
});

it('distinguishes a listening server from verified authentication and retries without a settings detour', async () => {
  calls.localAccess.mockResolvedValue({ ...fresh, listening: [5900] });
  await show();
  expect(document.body.textContent).toContain('Server detected · authentication not verified');
  expect(document.body.textContent).not.toContain('macOS requires you to turn on');
  expect(document.querySelector('[aria-label="Enable Screen sharing"]')).toBeNull();
  const retry = [...document.querySelectorAll('button')].find((button) => button.textContent.trim() === 'Check again');
  expect(retry).toBeDefined();
  retry?.click();
  await settle();
  expect(calls.localAccess).toHaveBeenLastCalledWith([22, 5900], 5900);
  expect(calls.settings).not.toHaveBeenCalled();
  expect(document.body.textContent).not.toContain('Ready in Anima');
});


it('reads saved hub access when reopening without requiring another enable action', async () => {
  calls.localAccess.mockResolvedValue({ ...fresh, configured: true, open: [22, 5900], ready: [22, 5900] });
  calls.publishedAccess.mockResolvedValue([22, 5900]);
  const target = document.createElement('div');
  document.body.append(target);
  app = mount(AccessSetup, { target });
  target.querySelector<HTMLButtonElement>('button')?.click();
  await settle();
  expect(document.querySelector('[aria-label="Enabled SSH"]')).not.toBeNull();
  expect(document.querySelector('[aria-label="Enabled Screen sharing"]')).not.toBeNull();
  expect(calls.publishAccess).not.toHaveBeenCalled();
  expect(calls.localAccess).toHaveBeenCalledExactlyOnceWith();
});

it('invalidates saved publication when a later opening cannot read the hub', async () => {
  calls.localAccess.mockResolvedValue({ ...fresh, configured: true, open: [22], ready: [22] });
  calls.publishedAccess.mockResolvedValueOnce([22]).mockRejectedValueOnce(new Error('Hub status unavailable. Retry.'));
  const target = document.createElement('div');
  document.body.append(target);
  app = mount(AccessSetup, { target });
  target.querySelector<HTMLButtonElement>('button')?.click();
  await settle();
  expect(document.body.textContent).toContain('Ready in Anima');
  document.querySelector<HTMLButtonElement>('[aria-label="Close"]')?.click();
  await settle();
  target.querySelector<HTMLButtonElement>('button')?.click();
  await settle();
  expect(calls.publishedAccess).toHaveBeenCalledTimes(2);
  expect(document.body.textContent).not.toContain('Ready in Anima');
  expect(document.body.textContent).toContain('Hub status unavailable. Retry.');
  expect(document.querySelector('[aria-label="Enable in Anima: SSH"]')?.getAttribute('aria-disabled')).toBe('false');
  expect(calls.publishAccess).not.toHaveBeenCalled();
  expect(calls.localAccess.mock.calls).toEqual([[], []]);
});


it('shows one persistent control per enabled protocol without applying setup defaults', async () => {
  calls.localAccess.mockResolvedValue({ ...fresh, configured: true, open: [22, 5900], ready: [22, 5900] });
  calls.publishedAccess.mockResolvedValue([22, 5900]);
  const target = document.createElement('div');
  document.body.append(target);
  app = mount(AccessSetup, { target, props: { inline: true } });
  await settle();
  expect(document.querySelector('[role="dialog"]')).toBeNull();
  expect(document.querySelector('[role="switch"]')).toBeNull();
  expect(document.body.textContent).toContain('Turn off SSH');
  expect(document.body.textContent).toContain('Turn off screen sharing');
  expect(calls.localAccess).toHaveBeenCalledExactlyOnceWith();
  expect(calls.publishAccess).not.toHaveBeenCalled();
  calls.localAccess.mockResolvedValue({ ...fresh, configured: true, selected: [5900], open: [5900], ready: [22, 5900] });
  calls.publishAccess.mockRejectedValueOnce(new Error('Hub update failed. Retry.'));
  [...target.querySelectorAll('button')].find((button) => button.textContent.trim() === 'Turn off SSH')?.click();
  await settle();
  expect(calls.linkPorts).toHaveBeenLastCalledWith({ port: 22, on: false });
  expect(calls.localAccess).toHaveBeenLastCalledWith();
  expect(calls.publishAccess.mock.calls[0]?.[2]).toBe(22);
  expect(document.body.textContent).toContain('Turn off screen sharing');
  const retry = [...target.querySelectorAll('button')].find((button) => button.textContent.trim() === 'Retry turning off');
  expect(retry).toBeDefined();
  retry?.click();
  await settle();
  expect(calls.linkPorts).toHaveBeenLastCalledWith({ port: 22, on: false });
  expect(calls.localAccess).toHaveBeenLastCalledWith();
  expect(calls.publishAccess).toHaveBeenCalledTimes(2);
  expect(document.body.textContent).not.toContain('Retry turning off');
});

it('keeps local off available on a headless machine when the selected hub differs', async () => {
  vi.stubGlobal('navigator', { userAgent: 'Linux' });
  calls.publishAccess.mockRejectedValue(new Error('Connect this computer to this hub.'));
  calls.localAccess.mockResolvedValue({ ...fresh, configured: true, desktop: false, selected: [3389, 5900], open: [3389, 5900], listening: [5900] });
  calls.publishedAccess.mockResolvedValue([5900]);
  calls.checkAccessHub.mockRejectedValue(new Error('Connect this computer to this hub.'));
  const target = document.createElement('div');
  document.body.append(target);
  app = mount(AccessSetup, { target, props: { inline: true } });
  await settle();
  const off = [...target.querySelectorAll('button')].find((button) => button.textContent.trim() === 'Turn off screen sharing');
  expect(off).toBeDefined();
  calls.localAccess.mockResolvedValue({ ...fresh, configured: true, desktop: false, selected: [3389], open: [3389], listening: [5900] });
  off?.click();
  await settle();
  expect(calls.linkPorts).toHaveBeenCalledExactlyOnceWith({ port: 5900, on: false });
  expect(calls.localAccess).toHaveBeenLastCalledWith();
  expect(calls.publishAccess.mock.calls[0]?.[2]).toBe(5900);
});
