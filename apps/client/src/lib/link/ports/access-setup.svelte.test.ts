// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { installAnimations } from '$lib/forms/animations.fixture';
import AccessSetup from './AccessSetup.svelte';

const calls = vi.hoisted(() => ({ localAccess: vi.fn(), publishAccess: vi.fn(), checkAccessHub: vi.fn() }));
vi.mock('$lib/desktop/link/access', () => ({ localAccess: calls.localAccess }));
vi.mock('./access', () => ({ publishAccess: calls.publishAccess, checkAccessHub: calls.checkAccessHub }));
vi.mock('$lib/hub/context', () => ({ useHub: () => ({}) }));
vi.mock('motion', async (original) => ({ ...(await original<typeof import('motion')>()), animate: (await import('$lib/motion/engine.fixture')).fakeRun }));

const fresh = { configured: false, desktop: true, username: 'operator', selected: [22, 5900], open: [], notes: [] };
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
  calls.localAccess.mockResolvedValueOnce({ ...fresh, configured: true, selected: [22], open: [22] });
  [...document.querySelectorAll('button')].find((button) => button.textContent.trim() === 'Continue')?.click();
  await settle();
  expect(calls.localAccess).toHaveBeenLastCalledWith([22]);
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
