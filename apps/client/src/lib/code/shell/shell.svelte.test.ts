// @vitest-environment jsdom
import { flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { browserCapabilities } from '$lib/platform/browser';
import type { Capabilities } from '$lib/platform/capabilities';
import TopBand from './TopBand.svelte';
import WindowControls from './WindowControls.svelte';
import { shellWindow, type WindowFrame } from '$lib/desktop/window/window';

const shell = vi.hoisted(() => ({
  window: {
    minimize: vi.fn(() => Promise.resolve()),
    toggleMaximize: vi.fn(() => Promise.resolve()),
    close: vi.fn(() => Promise.resolve()),
    isMaximized: vi.fn(() => Promise.resolve(false)),
    onResized: vi.fn(() => Promise.resolve(() => undefined)),
  },
  answers: { current: undefined as Capabilities | undefined },
}));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => shell.window }));
vi.mock('$lib/platform/platform', () => ({ capabilities: () => shell.answers.current }));

let app: ReturnType<typeof mount> | undefined;

function frame(maximised: { now: boolean }): WindowFrame & { resized: () => void; stopped: number } {
  const made = {
    resized: (): void => undefined,
    stopped: 0,
    minimise: vi.fn(() => Promise.resolve()),
    toggleMaximise: vi.fn(() => Promise.resolve()),
    close: vi.fn(() => Promise.resolve()),
    isMaximised: () => Promise.resolve(maximised.now),
    onResized: (listener: () => void) => {
      made.resized = listener;
      return Promise.resolve(() => {
        made.stopped += 1;
      });
    },
  };
  return made;
}

function button(name: string): HTMLButtonElement {
  const found = document.querySelector<HTMLButtonElement>(`button[aria-label="${name}"]`);
  if (found === null) throw new Error(`no button "${name}"`);
  return found;
}

async function settle(): Promise<void> {
  await tick();
  await Promise.resolve();
  flushSync();
}

beforeEach(() => {
  shell.answers.current = { ...browserCapabilities };
});

afterEach(() => {
  if (app !== undefined) void unmount(app);
  app = undefined;
  document.body.innerHTML = '';
});

describe('the window with no frame (TC2)', () => {
  it('TC2: the commands of the window take no argument from the web view', async () => {
    const current = shellWindow();
    await current.minimise();
    await current.toggleMaximise();
    await current.close();
    expect(shell.window.minimize).toHaveBeenCalledWith();
    expect(shell.window.toggleMaximize).toHaveBeenCalledWith();
    expect(shell.window.close).toHaveBeenCalledWith();
  });

  it('TC2: the three controls are buttons with names, and each runs its command', async () => {
    const made = frame({ now: false });
    app = mount(WindowControls, { target: document.body, props: { frame: made } });
    await settle();
    expect(document.querySelector('[role="group"]')?.getAttribute('aria-label')).toBe('Window');
    button('Minimise').click();
    button('Maximise').click();
    button('Close').click();
    expect(made.minimise).toHaveBeenCalledTimes(1);
    expect(made.toggleMaximise).toHaveBeenCalledTimes(1);
    expect(made.close).toHaveBeenCalledTimes(1);
  });

  it('TC2: the middle control follows the window, also when the size changed by a double click on the band', async () => {
    const state = { now: false };
    const made = frame(state);
    app = mount(WindowControls, { target: document.body, props: { frame: made } });
    await settle();
    expect(document.querySelector('button[aria-label="Restore"]')).toBeNull();
    state.now = true;
    made.resized();
    await settle();
    expect(button('Restore').title).toBe('Restore');
    void unmount(app);
    app = undefined;
    expect(made.stopped).toBe(1);
  });

  it('TC2: a command that fails leaves the controls as they are', async () => {
    const made = frame({ now: false });
    made.close = vi.fn(() => Promise.reject(new Error('refused')));
    app = mount(WindowControls, { target: document.body, props: { frame: made } });
    await settle();
    button('Close').click();
    await settle();
    expect(button('Close')).not.toBeNull();
  });
});

describe('the top band of Code (TC2)', () => {
  it('TC2: the band moves the window, and it draws the controls only for a window with no frame', async () => {
    app = mount(TopBand, { target: document.body, props: { holdsWindowControls: true } });
    await settle();
    expect(document.querySelector('header[data-tauri-drag-region]')).not.toBeNull();
    expect(document.querySelector('button[aria-label="Close"]')).toBeNull();
    void unmount(app);
    shell.answers.current = { ...browserCapabilities, framelessWindow: true };
    app = mount(TopBand, { target: document.body, props: { holdsWindowControls: true } });
    await settle();
    expect(button('Close')).not.toBeNull();
  });

  it('TC2: a band that is not the last one of its row draws no controls', async () => {
    shell.answers.current = { ...browserCapabilities, framelessWindow: true };
    app = mount(TopBand, { target: document.body, props: {} });
    await settle();
    expect(document.querySelector('button[aria-label="Close"]')).toBeNull();
  });

  it('TC2: on a Mac the first band leaves room for the window buttons', async () => {
    shell.answers.current = { ...browserCapabilities, overlayTitleBar: true };
    app = mount(TopBand, { target: document.body, props: { clearsWindowButtons: true } });
    await settle();
    expect(document.querySelector('header')?.classList.contains('clears')).toBe(true);
  });
});
