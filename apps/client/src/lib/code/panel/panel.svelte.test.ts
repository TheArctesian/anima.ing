// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { browserCapabilities } from '$lib/platform/browser';
import type { Capabilities } from '$lib/platform/capabilities';
import EmptyPanel from './EmptyPanel.svelte';
import PanelEdge from './PanelEdge.svelte';
import PanelTabBar from './PanelTabBar.svelte';
import PanelToggles from './PanelToggles.svelte';
import { NO_TABS, opened, type PanelTabs } from './tabs';
import { KEY_STEP, MIN_WIDTH } from './width';

const shell = vi.hoisted(() => ({ answers: { current: undefined as Capabilities | undefined } }));
vi.mock('$lib/platform/platform', () => ({ capabilities: () => shell.answers.current }));

let app: ReturnType<typeof mount> | undefined;
const two: PanelTabs = opened(opened(NO_TABS, { id: 'f', kind: 'files', title: 'Files' }), {
  id: 'a',
  kind: 'file',
  title: 'layout.svelte',
  path: 'src/routes/layout.svelte',
});

beforeEach(() => {
  shell.answers.current = { ...browserCapabilities };
});

afterEach(() => {
  if (app !== undefined) void unmount(app);
  app = undefined;
  document.body.innerHTML = '';
});

function press(target: Element, key: string): void {
  target.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
  flushSync();
}

describe('the empty right panel (TC5)', () => {
  it('TC5: a letter opens its surface, and a surface that is not available stays closed and says why', () => {
    const onopen = vi.fn();
    app = mount(EmptyPanel, { target: document.body, props: { onopen, unavailable: { terminal: 'This thread has no project.' } } });
    flushSync();
    expect(document.querySelector('h3')?.textContent).toBe('Open a surface');
    const list = document.querySelector('ul');
    if (!list) throw new Error('no list');
    press(list, 'd');
    press(list, 't');
    expect(onopen.mock.calls).toEqual([['diff']]);
    const terminal = [...document.querySelectorAll('button')].find((button) => button.textContent.includes('Terminal'));
    expect(terminal?.getAttribute('aria-disabled')).toBe('true');
    expect(terminal?.title).toBe('This thread has no project.');
    press(list, 'ArrowDown');
    press(list, 'ArrowDown');
    press(list, 'Enter');
    expect(onopen).toHaveBeenLastCalledWith('files');
  });
});

describe('the tab bar of the right panel (TC5)', () => {
  it('TC5: a tab and its close button are two buttons side by side, each with its name', () => {
    const onshow = vi.fn();
    const onclose = vi.fn();
    app = mount(PanelTabBar, { target: document.body, props: { state: two, onshow, onclose, onadd: vi.fn() } });
    flushSync();
    const tabs = [...document.querySelectorAll<HTMLButtonElement>('[role="tab"]')];
    expect(tabs.map((tab) => tab.getAttribute('aria-selected'))).toEqual(['false', 'true']);
    expect(tabs[1]?.title).toBe('src/routes/layout.svelte');
    expect(document.querySelector('[role="tab"] button')).toBeNull();
    tabs[0]?.click();
    document.querySelector<HTMLButtonElement>('button[aria-label="Close Files"]')?.click();
    expect(onshow).toHaveBeenCalledWith('f');
    expect(onclose).toHaveBeenCalledWith('f');
  });

  it('TC5: the arrows move between the tabs, Delete and the middle button close one', () => {
    const onshow = vi.fn();
    const onclose = vi.fn();
    app = mount(PanelTabBar, { target: document.body, props: { state: two, onshow, onclose, onadd: vi.fn() } });
    flushSync();
    const tabs = [...document.querySelectorAll('[role="tab"]')];
    if (!tabs[0] || !tabs[1]) throw new Error('no tabs');
    press(tabs[1], 'ArrowRight');
    expect(onshow).toHaveBeenLastCalledWith('f');
    press(tabs[1], 'Delete');
    tabs[0].dispatchEvent(new MouseEvent('auxclick', { button: 1, bubbles: true }));
    expect(onclose.mock.calls).toEqual([['a'], ['f']]);
  });

  it('TC5: the bar moves the window and keeps the room of the window controls only with no frame', () => {
    shell.answers.current = { ...browserCapabilities, framelessWindow: true };
    app = mount(PanelTabBar, { target: document.body, props: { state: NO_TABS, onshow: vi.fn(), onclose: vi.fn(), onadd: vi.fn() } });
    flushSync();
    const bar = document.querySelector('[data-code-band]');
    expect(bar?.getAttribute('data-tauri-drag-region')).toBe('deep');
    expect(bar?.classList.contains('keeps')).toBe(true);
    expect(document.querySelector('button[aria-label="Add panel surface"]')).not.toBeNull();
  });
});

describe('the edge of the right panel (TC5)', () => {
  it('TC5: the width can be changed with the keys, and the edge says its limits', () => {
    const onresize = vi.fn();
    app = mount(PanelEdge, { target: document.body, props: { width: 540, max: 740, onresize } });
    flushSync();
    const edge = document.querySelector('[role="separator"]');
    if (!edge) throw new Error('no edge');
    expect(edge.getAttribute('aria-valuemin')).toBe(String(MIN_WIDTH));
    expect(edge.getAttribute('aria-valuemax')).toBe('740');
    press(edge, 'ArrowLeft');
    press(edge, 'ArrowRight');
    press(edge, 'Home');
    press(edge, 'End');
    expect(onresize.mock.calls).toEqual([[540 + KEY_STEP], [540 - KEY_STEP], [740], [MIN_WIDTH]]);
  });
});

describe('the toggles of the panels in the top band', () => {
  const press = (name: string): HTMLButtonElement | null => document.querySelector<HTMLButtonElement>(`button[aria-label="${name}"]`);

  it('says which part shows, and asks for the part that is pressed', () => {
    const asked: string[] = [];
    app = mount(PanelToggles, { target: document.body, props: { open: { details: true, terminal: false, panel: false }, ontoggle: (part) => asked.push(part) } });
    flushSync();
    expect(press('Toggle thread details')?.getAttribute('aria-pressed')).toBe('true');
    expect(press('Toggle right panel')?.getAttribute('aria-pressed')).toBe('false');
    press('Toggle terminal drawer')?.click();
    press('Toggle right panel')?.click();
    expect(asked).toEqual(['terminal', 'panel']);
    expect(press('Maximize right panel')).toBeNull();
  });

  it('switches the terminal and the right panel off with no project, and offers Maximize while the panel is open', () => {
    app = mount(PanelToggles, { target: document.body, props: { open: { details: false, terminal: false, panel: true }, project: false, ontoggle: () => undefined, onmaximise: () => undefined } });
    flushSync();
    expect(press('Toggle terminal drawer')?.disabled).toBe(true);
    expect(press('Toggle right panel')?.disabled).toBe(true);
    expect(press('Toggle thread details')?.disabled).toBe(false);
    expect(press('Maximize right panel')).not.toBeNull();
  });
});
