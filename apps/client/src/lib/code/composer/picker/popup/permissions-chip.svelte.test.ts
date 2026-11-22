// @vitest-environment jsdom
import { flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, beforeAll, expect, it } from 'vitest';
import Card from '../../Card.svelte';
import { CAN, cannot } from '../../draft';
import type { ComposerEngine } from '../../engine';
import { FIXTURE_SOURCES } from '../../mentions/fixture';
import type { Choice } from '../catalogue';
import { pickerCatalogue, pickerChoice } from '../fixture';

let app: ReturnType<typeof mount> | undefined;

beforeAll(() => {
  window.matchMedia = (query: string) => ({ matches: false, media: query }) as MediaQueryList;
  Element.prototype.animate ??= () => ({ cancel: () => undefined, finished: Promise.resolve() }) as unknown as Animation;
});

afterEach(() => {
  if (app) void unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

/** The card over an engine that takes each choice; `more` changes what the engine can do. */
function card(contextStrip: boolean, more: Partial<ComposerEngine> = {}): Choice[] {
  const picks: Choice[] = [];
  const ok = () => Promise.resolve({ ok: true as const });
  const made = $state({ choice: pickerChoice });
  const engine: ComposerEngine = {
    phase: 'idle',
    ended: null,
    send: ok,
    interrupt: ok,
    attach: CAN,
    dictate: cannot('This build has no dictation.'),
    steer: CAN,
    queue: CAN,
    queued: [],
    enqueue: ok,
    requeue: ok,
    unqueue: ok,
    sendQueued: ok,
    pick: CAN,
    catalogue: pickerCatalogue,
    get choice() {
      return made.choice;
    },
    choose: (next) => {
      picks.push(next);
      made.choice = next;
      return ok();
    },
    sources: FIXTURE_SOURCES,
    ...more,
  };
  app = mount(Card, { target: document.body, props: { engine, contextStrip } });
  flushSync();
  return picks;
}

const chip = (): HTMLButtonElement | null => document.querySelector<HTMLButtonElement>('[data-slot="composer-context-strip"] button[aria-haspopup="dialog"]');
const mic = (): Element | null => document.querySelector('button[aria-label="Start voice input"]');

it('shows the permissions under the card of a chat that is not started, and changes the mode from there', async () => {
  const picks = card(true);
  expect(chip()?.textContent.trim()).toBe('Supervised');
  expect(chip()?.getAttribute('aria-label')).toBe('Permissions: Supervised');
  chip()?.click();
  await tick();
  flushSync();
  document.querySelector<HTMLButtonElement>('[data-slot="composer-context-strip"] [role="radio"][aria-label="Full access"]')?.click();
  await tick();
  flushSync();
  expect(picks).toEqual([{ ...pickerChoice, mode: 'full-access' }]);
  expect(chip()?.textContent.trim()).toBe('Full access');
  // The menu leaves with a transition that jsdom never ends: the chip says that it is closed.
  expect(chip()?.getAttribute('aria-expanded')).toBe('false');
});

it('has no strip for a chat that runs, and none when the engine changes no mode', () => {
  card(false);
  expect(document.querySelector('[data-slot="composer-context-strip"]')).toBeNull();
  if (app) void unmount(app);
  card(true, { pick: cannot('The runner changes no mode.') });
  expect(document.querySelector('[data-slot="composer-context-strip"]')).toBeNull();
});

it('draws the microphone only where dictation works', () => {
  card(false);
  expect(mic()).toBeNull();
  if (app) void unmount(app);
  card(false, { dictate: CAN });
  expect(mic()).not.toBeNull();
});
