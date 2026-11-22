// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ownsKeys, pageKey } from '$lib/agent-threads/keys';
import type { LiveEntry, OpenAsk } from '$lib/agent-threads/live/timeline';
import Timeline from '../timeline/Timeline.svelte';

// The conversation of a thread for a keyboard and a screen reader (accessibility findings 2, 3, 4 and 17).

const at = (seconds: number): Date => new Date(Date.UTC(2026, 10, 21, 9, 0, seconds));
const prompt = (text: string, seconds: number): LiveEntry => ({ kind: 'prompt', at: at(seconds), text, images: 0 });
const reply = (text: string, seconds: number): LiveEntry => ({ kind: 'reply', at: at(seconds), text });
const end = (seconds: number): LiveEntry => ({ kind: 'turn_end', at: at(seconds), stopped: false });

vi.mock('$lib/motion/preference', () => ({ motionPreference: () => ({ reduced: true }) }));

/** jsdom has no ResizeObserver: the follow of the end is not what these tests read. */
class NoResize {
  observe(): void {
    return undefined;
  }
  disconnect(): void {
    return undefined;
  }
}
vi.stubGlobal('ResizeObserver', NoResize);
Element.prototype.scrollTo = () => undefined;

let app: ReturnType<typeof mount> | undefined;

afterEach(async () => {
  if (app) await unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

const ASK: OpenAsk = {
  request: 'r1',
  ask: { kind: 'command', tool: 'Bash', subject: 'rm -rf build', diff: null, sessionRules: [], questions: [] },
};

interface Shown {
  entries: LiveEntry[];
  working: boolean;
  asks: OpenAsk[];
}

const ASK_TWO: OpenAsk = { request: 'r2', ask: { ...ASK.ask, subject: 'git push' } };

/** The timeline as a thread shows it; `quietMs` 0 unless a test reads the quiet time after the open. */
function shown(start: Shown, quietMs = 0, onDecide: (request: string) => void = () => undefined): Shown {
  const props = $state(start);
  app = mount(Timeline, {
    target: document.body,
    props: {
      get entries() {
        return props.entries;
      },
      get working() {
        return props.working;
      },
      get asks() {
        return props.asks;
      },
      onDecide,
      quietMs,
      clock: () => at(99),
    },
  });
  return props;
}

describe('the conversation for a keyboard and a screen reader', () => {
  it('finding 2: the arrows in the conversation and in its code scroll it; the page does not take them', () => {
    shown({ entries: [prompt('Go', 0), reply('Here:\n\n```\nls\n```', 3), end(3)], working: false, asks: [] });
    flushSync();
    const conversation = document.querySelector('[aria-label="Conversation"]');
    const code = document.querySelector('pre[aria-label="Code"]');
    expect(ownsKeys(conversation)).toBe(true);
    expect(ownsKeys(code)).toBe(true);
    const down = { key: 'ArrowDown', mod: false, shift: false, alt: false, typing: false };
    expect(pageKey(down, conversation)).toBeNull();
    expect(pageKey(down, document.body)).toEqual({ kind: 'move', by: 1 });
  });

  it('finding 3: the start and the end of a turn are said once, politely; what was there at the open is not', async () => {
    const props = shown({ entries: [prompt('Old', 0), reply('Old answer.', 1), end(1)], working: false, asks: [] });
    flushSync();
    const polite = document.querySelector('[aria-live="polite"]');
    expect(polite?.textContent).toBe('');
    props.entries = [...props.entries, prompt('Go', 5)];
    props.working = true;
    flushSync();
    await vi.waitFor(() => {
      expect(polite?.textContent).toBe('Agent is working.');
    });
    props.entries = [...props.entries, reply('The tests pass. Two files changed.', 9), end(9)];
    props.working = false;
    flushSync();
    await vi.waitFor(() => {
      expect(polite?.textContent).toBe('Agent replied: The tests pass.');
    });
  });

});

describe('the requests of the agent for a keyboard and a screen reader', () => {
  it('finding 4: a request of the agent is said at once and the focus goes to its Approve button', async () => {
    const props = shown({ entries: [prompt('Clean', 0)], working: true, asks: [] });
    flushSync();
    props.asks = [ASK];
    flushSync();
    await vi.waitFor(() => {
      expect(document.querySelector('[role="alert"]')?.textContent).toBe('Approval needed to run a command: rm -rf build');
    });
    await vi.waitFor(() => {
      expect(document.activeElement?.textContent.trim()).toBe('Approve');
    });
  });

  it('finding 4: a thread opened with a request that waits says nothing and leaves the focus where it is', () => {
    shown({ entries: [prompt('Clean', 0)], working: true, asks: [ASK] }, 60_000);
    flushSync();
    expect(document.querySelector('[role="alert"]')?.textContent).toBe('');
    expect(document.activeElement).toBe(document.body);
  });

  it('finding 4: two requests in one update are said together', async () => {
    const props = shown({ entries: [prompt('Ship', 0)], working: true, asks: [] });
    flushSync();
    props.asks = [ASK, ASK_TWO];
    flushSync();
    await vi.waitFor(() => {
      expect(document.querySelector('[role="alert"]')?.textContent).toBe('Approval needed to run a command: rm -rf build Approval needed to run a command: git push');
    });
  });

  it('finding 17: after the answer the focus is in the conversation, not lost with the row', async () => {
    const held: { props?: Shown } = {};
    const props = shown({ entries: [prompt('Clean', 0)], working: true, asks: [] }, 0, () => {
      if (held.props !== undefined) held.props.asks = [];
    });
    held.props = props;
    flushSync();
    props.asks = [ASK];
    flushSync();
    await vi.waitFor(() => {
      expect(document.activeElement?.textContent.trim()).toBe('Approve');
    });
    (document.activeElement as HTMLElement).click();
    flushSync();
    expect(document.activeElement?.getAttribute('aria-label')).toBe('Conversation');
  });
});
