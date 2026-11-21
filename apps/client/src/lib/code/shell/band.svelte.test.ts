// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import Breadcrumb from './Breadcrumb.svelte';
import HistoryButtons from './HistoryButtons.svelte';
import InboxToggle from './InboxToggle.svelte';

let app: ReturnType<typeof mount> | undefined;

afterEach(() => {
  if (app !== undefined) void unmount(app);
  app = undefined;
  document.body.innerHTML = '';
});

function button(name: string): HTMLButtonElement {
  const found = document.querySelector<HTMLButtonElement>(`button[aria-label="${name}"]`);
  if (found === null) throw new Error(`no button "${name}"`);
  return found;
}

describe('the path of the head of a thread (TC2)', () => {
  it('TC2: the path is "project / title", and the title is the heading of the thread', () => {
    app = mount(Breadcrumb, { target: document.body, props: { project: 'anima.ing', title: 'Fix the gates' } });
    flushSync();
    const items = [...document.querySelectorAll('nav[aria-label="Thread breadcrumb"] li')].map((item) =>
      item.textContent.trim(),
    );
    expect(items).toEqual(['anima.ing', '/', 'Fix the gates']);
    expect(document.querySelector('h2')?.textContent.trim()).toBe('Fix the gates');
  });

  it('TC2: a thread with no project says so, and a sub thread shows its parent', () => {
    const onparent = vi.fn();
    app = mount(Breadcrumb, { target: document.body, props: { parent: 'Plan the release', title: 'Read the logs', onparent } });
    flushSync();
    expect(document.querySelector('li')?.textContent.trim()).toBe('No project');
    button('Open parent thread: Plan the release').click();
    expect(onparent).toHaveBeenCalledTimes(1);
  });

  it('TC2: the project starts a new thread, and the title opens the actions at itself', () => {
    const onproject = vi.fn();
    const onactions = vi.fn();
    app = mount(Breadcrumb, {
      target: document.body,
      props: { project: 'anima.ing', title: 'Fix the gates', onproject, onactions },
    });
    flushSync();
    document.querySelector<HTMLButtonElement>('button[title="New thread in anima.ing"]')?.click();
    button('Thread actions for Fix the gates').click();
    expect(onproject).toHaveBeenCalledTimes(1);
    expect(onactions).toHaveBeenCalledWith(button('Thread actions for Fix the gates'));
  });

  it('TC2: a title of an agent is drawn as text', () => {
    app = mount(Breadcrumb, { target: document.body, props: { title: '<img src=x onerror=alert(1)>' } });
    flushSync();
    expect(document.querySelector('img')).toBeNull();
    expect(document.querySelector('h2')?.textContent.trim()).toBe('<img src=x onerror=alert(1)>');
  });
});

describe('the buttons of the band (TC2)', () => {
  it('TC2: back and forward are off when there is no page to go to, and name their keys', () => {
    const onback = vi.fn();
    const onforward = vi.fn();
    app = mount(HistoryButtons, {
      target: document.body,
      props: { canGoBack: true, canGoForward: false, onback, onforward, keys: { back: '⌘[', forward: '⌘]' } },
    });
    flushSync();
    button('Go back (⌘[)').click();
    button('Go forward (⌘])').click();
    expect(onback).toHaveBeenCalledTimes(1);
    expect(button('Go forward (⌘])').disabled).toBe(true);
    expect(onforward).not.toHaveBeenCalled();
  });

  it('TC2: the toggle of the thread list says if the list shows', () => {
    const ontoggle = vi.fn();
    app = mount(InboxToggle, { target: document.body, props: { open: true, ontoggle, key: '⌘B' } });
    flushSync();
    const toggle = button('Toggle the thread list (⌘B)');
    expect(toggle.getAttribute('aria-pressed')).toBe('true');
    toggle.click();
    expect(ontoggle).toHaveBeenCalledTimes(1);
  });
});
