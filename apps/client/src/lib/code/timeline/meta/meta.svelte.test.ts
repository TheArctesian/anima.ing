// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { WorkEntry } from '../turns';
import WorkRow from '../WorkRow.svelte';
import CopyButton from './CopyButton.svelte';
import { dayTime } from './stamp';

let app: ReturnType<typeof mount> | undefined;

/** jsdom has no ResizeObserver: whether a notice is cut off is measured only in a browser. */
class NoResize {
  observe(): void {
    return undefined;
  }
  disconnect(): void {
    return undefined;
  }
}
vi.stubGlobal('ResizeObserver', NoResize);
// jsdom has no Web Animations: the tooltip of Copy fades in with one.
Element.prototype.animate ??= () => ({ cancel: () => undefined, finished: Promise.resolve() }) as unknown as Animation;

afterEach(async () => {
  if (app) await unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

const NOW = new Date(2026, 10, 21, 15, 30);
const entry = (more: Partial<WorkEntry>): WorkEntry => ({ key: 'e', kind: 'notice', at: NOW, tool: null, text: 'A notice', running: false, stopped: false, ...more });

function row(shown: WorkEntry): void {
  app = mount(WorkRow, { target: document.body, props: { entry: shown, now: NOW } });
  flushSync();
}

describe('the rows of the work log as T3 Code draws them', () => {
  it('fidelity 3: a row with nothing more to show is no button and has no chevron', () => {
    row(entry({}));
    expect(document.querySelector('button')).toBeNull();
    expect(document.querySelector('.chevron')).toBeNull();
  });

  it('fidelity 3 and 22: a failed tool call opens, keeps its icon and says that it failed', () => {
    row(entry({ kind: 'command', text: '', tool: { name: 'Bash', kind: 'command', summary: 'ls', input: '{"command":"ls"}', patch: null, output: 'no such file', failed: true } }));
    const line = document.querySelector('button');
    expect(line?.getAttribute('aria-expanded')).toBe('false');
    expect(line?.getAttribute('aria-label'), 'the name says the failure, not the time of the hover').toMatch(/, failed$/);
    expect(document.querySelector('[role="img"]')?.getAttribute('aria-label')).toBe('Failed');
    line?.click();
    flushSync();
    expect(document.querySelector('[aria-label="Tool output"]')?.textContent).toBe('no such file');
  });

  it('fidelity 9: an opened thought is Markdown, not a box of plain text', () => {
    row(entry({ kind: 'reasoning', text: 'Check **this** first.' }));
    document.querySelector('button')?.click();
    flushSync();
    expect(document.querySelector('.thought strong')?.textContent).toBe('this');
    const thought = document.querySelector<HTMLElement>('[role="group"][aria-label="Thought"]');
    expect(thought?.tabIndex, 'a long thought scrolls, so a keyboard can reach it').toBe(0);
  });

  it('a press of Copy keeps the focus on the button while the check shows', async () => {
    Object.defineProperty(navigator, 'clipboard', { value: { writeText: () => Promise.resolve() }, configurable: true });
    app = mount(CopyButton, { target: document.body, props: { text: 'hello' } });
    flushSync();
    const copy = document.querySelector<HTMLButtonElement>('button');
    copy?.focus();
    copy?.click();
    await vi.waitFor(() => {
      expect(copy?.getAttribute('aria-label')).toBe('Copied');
    });
    expect(copy?.disabled).toBe(false);
    expect(document.activeElement).toBe(copy);
  });
});

describe('the times of the timeline (fidelity 13)', () => {
  it('says the time today, "yesterday at" the day before, then the date', () => {
    expect(dayTime(new Date(2026, 10, 21, 9, 5), NOW)).not.toMatch(/yesterday|\//);
    expect(dayTime(new Date(2026, 10, 20, 9, 5), NOW)).toMatch(/^yesterday at /);
    expect(dayTime(new Date(2025, 0, 2, 9, 5), NOW)).toMatch(/2025/);
  });
});
