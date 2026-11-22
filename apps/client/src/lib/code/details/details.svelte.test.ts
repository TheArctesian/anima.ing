// @vitest-environment jsdom
import { flushSync, mount, unmount, type ComponentProps } from 'svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import DetailsCard from './DetailsCard.svelte';
import FindBar from './FindBar.svelte';
import type { Details } from './model';

let app: ReturnType<typeof mount> | undefined;

beforeAll(() => {
  Element.prototype.animate ??= () => ({ cancel: () => undefined, finished: Promise.resolve() }) as unknown as Animation;
  Element.prototype.scrollIntoView ??= () => undefined;
});

afterEach(() => {
  if (app !== undefined) void unmount(app);
  app = undefined;
  document.body.innerHTML = '';
});

const full: Details = {
  runContext: { environments: [{ id: 'nas', label: 'nas', kind: 'server' }], environment: 'nas', mode: 'local', path: '/srv/app', locked: false },
  openIn: { editors: [{ id: 'zed', label: 'Zed' }], preferred: 'zed' },
  scripts: { scripts: [{ id: 'dev', name: 'dev', icon: 'play' }], primary: 'dev' },
  branch: { name: 'fix/login' },
  pullRequests: [
    { number: 12, title: 'Fix the login', state: 'open', checks: { done: 3, total: 3, failed: 0 }, canMerge: true },
    { number: 9, title: 'Older', state: 'merged' },
  ],
  servers: [{ id: 's', port: 5173, startedBy: 'agent', canStop: true }],
  related: [
    { id: 'p', kind: 'parent', title: 'Plan the login', status: 'done' },
    { id: 'a', kind: 'subagent', title: 'Run the tests', status: 'running', seconds: 30, canStop: true },
    { id: 'c', kind: 'side_chat', title: 'A question', status: 'idle' },
  ],
  usage: { limits: [{ label: 'Weekly', left: 8 }] },
  context: { used: 90_000, max: 200_000 },
};

function card(details: Details, more: Partial<ComponentProps<typeof DetailsCard>> = {}): HTMLElement {
  app = mount(DetailsCard, { target: document.body, props: { details, choices: {}, onChoices: () => undefined, ...more } });
  flushSync();
  return document.body;
}

const named = (root: ParentNode, name: string): HTMLElement | null => root.querySelector<HTMLElement>(`[aria-label="${name}"]`);

describe('the details card', () => {
  it('draws only the sections that have content, with a heading only where T3 Code has one', () => {
    const root = card(full);
    expect([...root.querySelectorAll('section')].map((section) => section.getAttribute('aria-label'))).toEqual(['Workspace', 'Version Control', 'Servers', 'Lineage', 'Side chats']);
    expect([...root.querySelectorAll('h3')].map((heading) => heading.textContent)).toEqual(['Servers', 'Lineage · 1 running', 'Side chats']);
    expect(root.textContent).not.toContain('Choose what to show');
  });

  it('shows the meters only when the person switched them on', () => {
    const root = card(full, { choices: { usage: true, context: true } });
    expect(named(root, 'Weekly: 8% left')).not.toBeNull();
    expect(named(root, 'Context: 45% used')?.textContent).toContain('90k / 200k');
  });

  it('switches a control off when the engine has no action for it, and calls the action when it has', () => {
    const merge = vi.fn();
    const root = card(full, { on: { pullRequestAction: merge } });
    const open = named(root, 'Pull request #12, open: Fix the login');
    expect(open?.getAttribute('aria-disabled')).toBe('true');
    const button = [...root.querySelectorAll('button')].find((each) => each.textContent.trim() === 'Merge');
    button?.click();
    expect(merge).toHaveBeenCalledWith(12, 'merge');
  });

  it('shows one pull request and the rest after "Show 1 more"', () => {
    const root = card(full);
    expect(named(root, 'Pull request #9, merged: Older')).toBeNull();
    [...root.querySelectorAll('button')].find((each) => each.textContent.includes('Show 1 more'))?.click();
    flushSync();
    expect(named(root, 'Pull request #9, merged: Older')).not.toBeNull();
  });

  it('hides the run context and the related threads when it is short', () => {
    const root = card(full, { density: 'compact' });
    expect(named(root, 'Run context: nas, Current checkout')).toBeNull();
    expect(named(root, 'Servers')).toBeNull();
    expect(named(root, 'Open in editor')).not.toBeNull();
  });

  it('is one row "Choose what to show" when it is empty, and its menu switches an item', () => {
    const changed = vi.fn();
    const root = card({}, { onChoices: changed });
    [...root.querySelectorAll('button')].find((each) => each.textContent.includes('Choose what to show'))?.click();
    flushSync();
    const boxes = [...root.querySelectorAll('[role="menuitemcheckbox"]')];
    expect(boxes).toHaveLength(12);
    expect(boxes.filter((box) => box.getAttribute('aria-checked') === 'true')).toHaveLength(10);
    (boxes.at(-1) as HTMLElement).click();
    expect(changed).toHaveBeenCalledWith({ context: true });
  });

  it('opens the picker of refs and picks with the keys', () => {
    const pick = vi.fn();
    const root = card({ branch: { name: 'main' } }, { refs: [{ name: 'main', current: true }, { name: 'fix/login' }], on: { pickRef: pick } });
    named(root, 'Branch: main')?.click();
    flushSync();
    const field = root.querySelector<HTMLInputElement>('[role="combobox"]');
    expect([...root.querySelectorAll('[role="option"]')].map((option) => option.textContent.trim())).toEqual(['main current', 'fix/login']);
    field?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    field?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    expect(pick).toHaveBeenCalledWith('fix/login');
  });
});

describe('the find bar', () => {
  const noop = (): void => undefined;

  it('counts the matches, and Enter and Shift+Enter walk them', () => {
    const next = vi.fn();
    const previous = vi.fn();
    app = mount(FindBar, { target: document.body, props: { query: 'log', find: { kind: 'matches', index: 3, total: 17 }, onQuery: noop, onNext: next, onPrevious: previous, onRetry: noop, onClose: noop } });
    flushSync();
    expect(document.querySelector('.count')?.textContent).toBe('3/17');
    const field = document.querySelector('input');
    field?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    field?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', shiftKey: true, bubbles: true }));
    expect([next.mock.calls.length, previous.mock.calls.length]).toEqual([1, 1]);
  });

  it('says 0/0 and switches the arrows off when nothing matches, and offers Retry after a failure', () => {
    app = mount(FindBar, { target: document.body, props: { query: 'zz', find: { kind: 'matches', index: 0, total: 0 }, onQuery: noop, onNext: noop, onPrevious: noop, onRetry: noop, onClose: noop } });
    flushSync();
    expect(document.querySelector('.count')?.textContent).toBe('0/0');
    expect(named(document, 'Next match')?.hasAttribute('disabled')).toBe(true);
    void unmount(app);
    app = mount(FindBar, { target: document.body, props: { query: 'zz', find: { kind: 'failed' }, onQuery: noop, onNext: noop, onPrevious: noop, onRetry: noop, onClose: noop } });
    flushSync();
    expect(document.body.textContent).toContain('Search failed');
    expect(document.body.textContent).toContain('Retry');
  });
});
