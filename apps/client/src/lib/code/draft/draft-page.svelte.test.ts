// @vitest-environment jsdom
import { flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, beforeAll, expect, it, vi } from 'vitest';
import type { LiveThreads } from '$lib/agent-threads/live/live.svelte';
import { outcomeFrom, type Action } from '$lib/agent-threads/live/run';
import { draftCatalogue, FIRST_CHOICE } from './choice';
import DraftThread from './DraftThread.svelte';
import { ThreadDraft } from './thread-draft.svelte';
// The runner's answer to `models` on a Mac with Claude Code and Codex: `anima agents models --json` (daemon 0.10.72),
// put in the socket's `outcome` form. Claude Code lists its own "default".
import REAL_MODELS_ANSWER from './real-models-answer.json';

let app: ReturnType<typeof mount> | undefined;

beforeAll(() => {
  window.matchMedia = (query: string) => ({ matches: false, media: query, addEventListener: () => undefined, removeEventListener: () => undefined }) as unknown as MediaQueryList;
  Element.prototype.animate ??= () => ({ cancel: () => undefined, finished: Promise.resolve() }) as unknown as Animation;
});

afterEach(() => {
  if (app) void unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

/** A runner that answers `models` as this Mac's did, through the page's own reading of the answer; `silent` never answers. */
function realRunner(calls: Action[], silent = false): LiveThreads {
  return {
    call: (action: Action) => {
      calls.push(action);
      if (silent) return new Promise(() => undefined);
      const outcome = action.call === 'models' ? outcomeFrom(REAL_MODELS_ANSWER) : { outcome: 'ok' as const };
      return Promise.resolve(outcome === null ? { ok: false, words: 'did not read' } : { ok: true, outcome });
    },
  } as unknown as LiveThreads;
}

async function openPage(live: LiveThreads): Promise<void> {
  app = mount(DraftThread, { target: document.body, props: { draft: new ThreadDraft('/work/anima'), live, projects: [], onStarted: () => undefined } });
  flushSync();
  await tick();
  await tick();
  flushSync();
}

function togglePicker(): void {
  document.querySelector<HTMLButtonElement>('button[aria-haspopup="dialog"]')?.click();
  flushSync();
}

it("lists this Mac's real models in the draft page's picker, Claude Code's own default once", async () => {
  const calls: Action[] = [];
  await openPage(realRunner(calls));
  togglePicker();
  const names = [...document.querySelectorAll('[role="option"] .name')].map((row) => row.firstChild?.textContent);
  expect(names).toContain('Opus 5.5');
  expect(names.filter((name) => name?.includes('Default') || name?.includes('default'))).toHaveLength(1);
  expect(document.querySelector('.notice')).toBeNull();
});

it('asks for the models again each time the picker opens, and keeps the list shown meanwhile', async () => {
  const calls: Action[] = [];
  await openPage(realRunner(calls));
  togglePicker();
  togglePicker();
  togglePicker();
  expect(calls.filter((call) => call.call === 'models')).toHaveLength(3);
  expect(document.querySelectorAll('[role="option"]').length).toBeGreaterThan(2);
});

it('counts the seconds while the runner does not answer', async () => {
  vi.useFakeTimers();
  try {
    await openPage(realRunner([], true));
    togglePicker();
    vi.advanceTimersByTime(4000);
    flushSync();
    expect(document.querySelector('.notice')?.textContent).toMatch(/Models load when the programs answer\.\s*Waited 4 s/);
  } finally {
    vi.useRealTimers();
  }
});

it('offers Ultra for a Codex model that lists it, and only there (the runner takes "ultra" since threads/codex-rollout-items)', () => {
  const answer = outcomeFrom(REAL_MODELS_ANSWER);
  if (answer?.outcome !== 'models') throw new Error('the capture did not read');
  const { catalogue } = draftCatalogue(FIRST_CHOICE, { state: 'listed', lists: answer.models });
  const levels = (id: string) => catalogue.models.find((model) => model.id === id)?.efforts.map((effort) => effort.label);
  expect(levels('gpt-6.2-sol')).toEqual(['Low', 'Medium', 'High', 'Extra High', 'Max', 'Ultra']);
  expect(catalogue.models.filter((model) => model.provider === 'claude_code').flatMap((model) => model.efforts.map((effort) => effort.id))).not.toContain('ultra');
});
