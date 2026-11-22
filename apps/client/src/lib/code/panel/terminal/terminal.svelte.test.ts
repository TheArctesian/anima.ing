// @vitest-environment jsdom
import { createRawSnippet, flushSync, mount, unmount, type ComponentProps } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { NO_TERMINALS, added, type TerminalGroup } from './group';
import TerminalSurface from './TerminalSurface.svelte';

type Props = ComponentProps<typeof TerminalSurface>;
let app: ReturnType<typeof mount> | undefined;

afterEach(() => {
  if (app !== undefined) void unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

const of = (...ids: string[]): TerminalGroup => ids.reduce((group, id) => added(group, { id, label: `zsh ${id}` }, 'horizontal'), NO_TERMINALS);
const pane = createRawSnippet<[{ id: string }, boolean]>((terminal, active) => ({
  render: () => `<pre data-shell="${terminal().id}" data-active="${String(active())}"></pre>`,
}));

function draw(props: Partial<Props> & { group: TerminalGroup }): Record<'onsplit' | 'onnew' | 'onactivate' | 'onclose', ReturnType<typeof vi.fn>> {
  const on = { onsplit: vi.fn(), onnew: vi.fn(), onactivate: vi.fn(), onclose: vi.fn() };
  app = mount(TerminalSurface, { target: document.body, props: { pane, ...on, ...props } });
  flushSync();
  return on;
}

const button = (name: string): HTMLButtonElement | null => document.querySelector<HTMLButtonElement>(`button[aria-label="${name}"]`);

describe('the terminal surface', () => {
  it('offers a new terminal when the tab has none', () => {
    const on = draw({ group: NO_TERMINALS, keys: { new: '⌘N' } });
    expect(document.body.textContent).toContain('No terminal sessions for this thread yet.');
    [...document.querySelectorAll('button')].find((each) => each.textContent === 'New Terminal (⌘N)')?.click();
    expect(on.onnew).toHaveBeenCalledOnce();
  });

  it('gives one terminal the four actions and no list', () => {
    const on = draw({ group: of('a'), keys: { split_horizontal: '⌘D' } });
    expect(document.querySelector('ul')).toBeNull();
    button('Split Terminal Horizontally (⌘D)')?.click();
    button('Split Terminal Vertically')?.click();
    button('Close Terminal')?.click();
    expect(on.onsplit.mock.calls).toEqual([['horizontal'], ['vertical']]);
    expect(on.onclose).toHaveBeenCalledWith('a');
  });

  it('lists the terminals of a split and tells each pane whether it is active', () => {
    const on = draw({ group: of('a', 'b') });
    expect([...document.querySelectorAll('[data-shell]')].map((each) => each.getAttribute('data-active'))).toEqual(['false', 'true']);
    expect(document.querySelector('li [aria-current="true"]')?.textContent.trim()).toBe('zsh b');
    document.querySelector<HTMLElement>('[data-terminal="a"]')?.dispatchEvent(new Event('pointerdown', { bubbles: true }));
    button('Close zsh a')?.click();
    expect(on.onactivate).toHaveBeenCalledWith('a');
    expect(on.onclose).toHaveBeenCalledWith('a');
  });

  it('turns the splits off at four terminals and says why', () => {
    const on = draw({ group: of('a', 'b', 'c', 'd') });
    const split = button('Split Terminal Horizontally (max 4 per group)');
    expect(split?.getAttribute('aria-disabled')).toBe('true');
    split?.click();
    button('New Terminal')?.click();
    expect(on.onsplit).not.toHaveBeenCalled();
    expect(on.onnew).toHaveBeenCalledOnce();
  });

  it('turns each action off for a person who may not operate terminals', () => {
    const on = draw({ group: of('a', 'b'), permitted: false });
    const actions = [...document.querySelectorAll<HTMLButtonElement>('[role="toolbar"] button')];
    expect(actions.map((each) => each.getAttribute('aria-disabled'))).toEqual(['true', 'true', 'true', 'true']);
    for (const each of actions) each.click();
    expect(button('Close zsh a')).toBeNull();
    expect([on.onsplit, on.onnew, on.onclose].some((each) => each.mock.calls.length > 0)).toBe(false);
  });
});
