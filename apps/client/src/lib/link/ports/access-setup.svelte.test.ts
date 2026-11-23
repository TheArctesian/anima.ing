// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { installAnimations } from '$lib/forms/animations.fixture';
import AccessSetup from './AccessSetup.svelte';

const calls = vi.hoisted(() => ({ localAccess: vi.fn(), publishAccess: vi.fn(), checkAccessHub: vi.fn(), settings: vi.fn() }));
vi.mock('$lib/desktop/link/access', () => ({ localAccess: calls.localAccess }));
vi.mock('./access', () => ({ publishAccess: calls.publishAccess, checkAccessHub: calls.checkAccessHub }));
vi.mock('$lib/desktop/system-settings', () => ({ openSharingSettings: calls.settings }));
vi.mock('$lib/hub/context', () => ({ useHub: () => ({}) }));
vi.mock('motion', async (original) => ({ ...(await original<typeof import('motion')>()), animate: (await import('$lib/motion/engine.fixture')).fakeRun }));

const fresh: import('$lib/desktop/link/access').Access = { configured: false, desktop: true, username: 'operator', selected: [22, 5900], open: [], ready: [], notes: [] };
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

it('offers defaults before making changes and submits a setup opt-out', async () => {
  await show();
  const switches = [...document.querySelectorAll<HTMLButtonElement>('[role="switch"]')];
  expect(switches.map((button) => button.getAttribute('aria-checked'))).toEqual(['true', 'true']);
  expect(calls.localAccess).toHaveBeenCalledExactlyOnceWith();
  switches[1]?.click();
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
