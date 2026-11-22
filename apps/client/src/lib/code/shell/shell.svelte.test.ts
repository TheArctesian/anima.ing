// @vitest-environment jsdom
import { flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { browserCapabilities } from '$lib/platform/browser';
import type { Capabilities } from '$lib/platform/capabilities';
import TopBand from './TopBand.svelte';
import WindowControls from './WindowControls.svelte';
import WindowStrip from './WindowStrip.svelte';
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
  it('TC2: each control sends its one command of the window', async () => {
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
    button('Minimise window').click();
    button('Maximise window').click();
    button('Close window').click();
    expect(made.minimise).toHaveBeenCalledTimes(1);
    expect(made.toggleMaximise).toHaveBeenCalledTimes(1);
    expect(made.close).toHaveBeenCalledTimes(1);
  });

  it('TC2: the middle control follows the window, also when the size changed by a double click on the band', async () => {
    const state = { now: false };
    const made = frame(state);
    app = mount(WindowControls, { target: document.body, props: { frame: made } });
    await settle();
    expect(document.querySelector('button[aria-label="Restore window"]')).toBeNull();
    state.now = true;
    made.resized();
    await settle();
    expect(button('Restore window').title).toBe('Restore');
    void unmount(app);
    app = undefined;
    expect(made.stopped).toBe(1);
  });

});

describe('the window controls and the size of the window (TC2)', () => {
  it('TC2: while one question is open a second change asks once more, so the last answer is of the last size', async () => {
    const answers: ((now: boolean) => void)[] = [];
    const made = frame({ now: false });
    made.isMaximised = () => new Promise<boolean>((resolve) => answers.push(resolve));
    app = mount(WindowControls, { target: document.body, props: { frame: made } });
    await settle();
    made.resized();
    made.resized();
    expect(answers).toHaveLength(1);
    answers[0]?.(true);
    await settle();
    expect(answers).toHaveLength(2);
    answers[1]?.(false);
    await settle();
    expect(document.querySelector('button[aria-label="Restore window"]')).toBeNull();
    expect(button('Maximise window')).not.toBeNull();
  });

  it('TC2: a command that fails leaves the controls as they are', async () => {
    const made = frame({ now: false });
    made.close = vi.fn(() => Promise.reject(new Error('refused')));
    app = mount(WindowControls, { target: document.body, props: { frame: made } });
    await settle();
    button('Close window').click();
    await settle();
    expect(button('Close window')).not.toBeNull();
  });
});

describe('the top band of Code (TC2)', () => {
  it('TC2: the band moves the window, tells the layout that the page has its own top, and draws no window controls', async () => {
    shell.answers.current = { ...browserCapabilities, framelessWindow: true };
    app = mount(TopBand, { target: document.body, props: { clearsWindowControls: true } });
    await settle();
    const band = document.querySelector('[data-code-band]');
    expect(band?.getAttribute('data-tauri-drag-region')).toBe('deep');
    expect(band?.classList.contains('keeps')).toBe(true);
    expect(document.querySelector('button')).toBeNull();
    expect(document.querySelector('header')).toBeNull();
  });

  it('TC2: a band keeps no room for the window controls where the system draws them, or when it is not the last', async () => {
    app = mount(TopBand, { target: document.body, props: { clearsWindowControls: true } });
    await settle();
    expect(document.querySelector('.band')?.classList.contains('keeps')).toBe(false);
    void unmount(app);
    shell.answers.current = { ...browserCapabilities, framelessWindow: true };
    app = mount(TopBand, { target: document.body, props: {} });
    await settle();
    expect(document.querySelector('.band')?.classList.contains('keeps')).toBe(false);
  });

  it('TC2: on a Mac the first band leaves room for the window buttons', async () => {
    shell.answers.current = { ...browserCapabilities, overlayTitleBar: true };
    app = mount(TopBand, { target: document.body, props: { clearsWindowButtons: true } });
    await settle();
    expect(document.querySelector('.band')?.classList.contains('clears')).toBe(true);
  });
});

describe('the strip at the top of a window with no frame (TC2)', () => {
  it('TC2: where the system draws the top of the window the strip is nothing', async () => {
    app = mount(WindowStrip, { target: document.body, props: {} });
    await settle();
    expect(document.body.querySelector('.strip')).toBeNull();
    expect(document.querySelector('button')).toBeNull();
  });

  it('TC2: with no frame the strip moves the window and holds the three controls, once', async () => {
    shell.answers.current = { ...browserCapabilities, framelessWindow: true };
    app = mount(WindowStrip, { target: document.body, props: { bare: true } });
    await settle();
    const strip = document.querySelector('.strip');
    expect(strip?.hasAttribute('data-tauri-drag-region')).toBe(true);
    expect(strip?.classList.contains('bare')).toBe(true);
    expect(document.querySelectorAll('button')).toHaveLength(3);
  });
});
