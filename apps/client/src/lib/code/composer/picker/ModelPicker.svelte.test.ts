// @vitest-environment jsdom
import { flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, beforeAll, expect, it } from 'vitest';
import { CAN, cannot, type Can } from '../draft';
import type { Choice } from './catalogue';
import { pickerCatalogue, pickerChoice } from './fixture';
import ModelPicker from './ModelPicker.svelte';

let app: ReturnType<typeof mount> | undefined;

beforeAll(() => {
  /* jsdom has no media queries and no element animations; the picker asks for a coarse pointer and fades its popup. */
  window.matchMedia = (query: string) => ({ matches: false, media: query }) as MediaQueryList;
  Element.prototype.animate ??= () => ({ cancel: () => undefined, finished: Promise.resolve() }) as unknown as Animation;
});

afterEach(() => {
  if (app) void unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

function show(can: Can = CAN, open = false): { picks: Choice[]; trigger: HTMLButtonElement } {
  const picks: Choice[] = [];
  const props = $state({ catalogue: pickerCatalogue, value: pickerChoice, can, open, onChange: (next: Choice) => {
    picks.push(next);
    props.value = next;
  } });
  app = mount(ModelPicker, { target: document.body, props });
  flushSync();
  const trigger = document.querySelector<HTMLButtonElement>('button[aria-haspopup="dialog"]');
  if (trigger === null) throw new Error('no trigger');
  return { picks, trigger };
}

const one = (selector: string): HTMLElement => {
  const found = document.querySelector<HTMLElement>(selector);
  if (found === null) throw new Error(`nothing matches ${selector}`);
  return found;
};
const texts = (selector: string): string[] => [...document.querySelectorAll(selector)].map((element) => element.textContent.replace(/\s+/gu, ' ').trim());
const key = (target: Element, name: string, held: KeyboardEventInit = {}): boolean => target.dispatchEvent(new KeyboardEvent('keydown', { key: name, bubbles: true, cancelable: true, ...held }));

async function settle(): Promise<void> {
  await tick();
  flushSync();
}

it('03 A.2.1, A.3.1: the trigger names the model and its effort, and opens the popup with focus in the search field', async () => {
  const { trigger } = show();
  expect([trigger.getAttribute('aria-label'), trigger.getAttribute('aria-expanded')]).toEqual(['Opus 5, High · Thinking', 'false']);
  expect(trigger.textContent.replace(/\s+/gu, ' ').trim()).toBe('Opus 5 High');
  trigger.click();
  await settle();
  expect(trigger.getAttribute('aria-expanded')).toBe('true');
  expect(document.activeElement).toBe(one('[role="combobox"]'));
  expect(one('[role="toolbar"] [aria-pressed="true"]').getAttribute('aria-label')).toBe('Favorites');
  expect(texts('[role="option"]').map((text) => text.split(',')[0])).toEqual(['Opus 5', 'Sonnet 5', 'GPT-6 Codex']);
});

it('03 A.2.5: Escape closes the popup and gives focus back to the trigger; a press outside closes it', async () => {
  const { trigger } = show();
  trigger.click();
  await settle();
  key(one('[role="combobox"]'), 'Escape');
  await settle();
  expect(trigger.getAttribute('aria-expanded')).toBe('false');
  expect(document.activeElement).toBe(trigger);
  trigger.click();
  await settle();
  document.body.dispatchEvent(new Event('pointerdown', { bubbles: true }));
  await settle();
  expect(trigger.getAttribute('aria-expanded')).toBe('false');
});

it('leaves each key to an input method that composes in the search field: the popup stays, the lit row and the model too', async () => {
  const { picks, trigger } = show();
  trigger.click();
  await settle();
  const search = one('[role="combobox"]');
  const lit = search.getAttribute('aria-activedescendant');
  const composing = [{ isComposing: true }, { keyCode: 229 }];
  const keys: [string, KeyboardEventInit][] = [['Escape', {}], ['Enter', {}], ['ArrowDown', {}], ['ArrowLeft', {}], ['2', { metaKey: true }], ['ArrowDown', { metaKey: true, shiftKey: true }]];
  const taken = composing.flatMap((how) => keys.map(([name, held]) => !key(search, name, { ...held, ...how })));
  await settle();
  expect(taken).not.toContain(true);
  expect(trigger.getAttribute('aria-expanded')).toBe('true');
  expect(document.activeElement).toBe(search);
  expect(search.getAttribute('aria-activedescendant')).toBe(lit);
  expect(one('[role="toolbar"] [aria-pressed="true"]').getAttribute('aria-label')).toBe('Favorites');
  expect(picks).toEqual([]);
  key(search, 'Escape');
  await settle();
  expect(trigger.getAttribute('aria-expanded')).toBe('false');
});

it('closes when focus goes on out of the trigger and the popup, as after Tab, and not when it stays or goes to nothing', async () => {
  const { trigger } = show();
  const outside = document.body.appendChild(document.createElement('button'));
  const leave = (to: Element | null): boolean => one('[role="combobox"]').dispatchEvent(new FocusEvent('focusout', { bubbles: true, relatedTarget: to }));
  trigger.click();
  await settle();
  leave(null);
  leave(one('[role="radio"]'));
  leave(trigger);
  await settle();
  expect(trigger.getAttribute('aria-expanded')).toBe('true');
  leave(outside);
  await settle();
  expect(trigger.getAttribute('aria-expanded')).toBe('false');
});

it('keeps each arrow of the rail and of the speeds from the keys of the page under the popup', async () => {
  show(CAN, true);
  await settle();
  const escaped: string[] = [];
  const page = new AbortController();
  window.addEventListener('keydown', (event) => escaped.push(event.key), { signal: page.signal });
  const rail = [...document.querySelectorAll<HTMLElement>('[role="toolbar"] button')];
  rail[0]?.focus();
  for (const name of ['ArrowDown', 'ArrowUp', 'ArrowUp', 'ArrowLeft']) key(document.activeElement ?? document.body, name);
  expect(document.activeElement).toBe(rail.at(-1));
  key(rail[0] as HTMLElement, 'ArrowRight');
  expect(document.activeElement).toBe(one('[role="combobox"]'));
  const speeds = [...document.querySelectorAll<HTMLElement>('[aria-label="Speed"] [role="radio"]')];
  speeds[0]?.focus();
  key(speeds[0] as HTMLElement, 'ArrowDown');
  expect(document.activeElement).toBe(speeds[1]);
  key(speeds[1] as HTMLElement, 'ArrowUp');
  key(speeds[0] as HTMLElement, 'ArrowRight');
  key(speeds[1] as HTMLElement, 'ArrowLeft');
  expect(document.activeElement).toBe(speeds[0]);
  page.abort();
  expect(escaped).toEqual([]);
});

it('03 A.2.5: Down and Enter pick the next model, the popup stays open and focus goes to its effort levels', async () => {
  const { picks, trigger } = show();
  trigger.click();
  await settle();
  const search = one('[role="combobox"]');
  key(search, 'ArrowDown');
  flushSync();
  expect(one(`#${CSS.escape(search.getAttribute('aria-activedescendant') ?? '')}`).textContent).toContain('Sonnet 5');
  key(search, 'Enter');
  await settle();
  expect(picks).toEqual([{ ...pickerChoice, model: 'sonnet-5' }]);
  expect(trigger.getAttribute('aria-expanded')).toBe('true');
  expect([document.activeElement?.getAttribute('role'), document.activeElement?.getAttribute('aria-checked')]).toEqual(['radio', 'true']);
  expect(one('[role="status"]:not(p)').textContent).toBe('Model: Sonnet 5');
});

it('03 A.2.4, A.2.5: a search hides the rail and lists all providers; the jump key picks; a model that cannot be picked is not', async () => {
  const { picks, trigger } = show();
  trigger.click();
  await settle();
  const search = one('[role="combobox"]') as HTMLInputElement;
  search.value = 'codex';
  search.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
  expect(document.querySelector('[role="toolbar"]')).toBeNull();
  expect(texts('[role="option"] .name').map((text) => text.split(',')[0])).toEqual(['GPT-6 Codex', 'GPT-6 Codex Mini', 'GPT-6 Codex Preview']);
  one('[role="option"][aria-disabled="true"]').click();
  await settle();
  expect(picks).toEqual([]);
  key(search, '2', { metaKey: true });
  await settle();
  expect(picks.map((pick) => [pick.provider, pick.model, pick.effort, pick.speed])).toEqual([['codex', 'gpt-6-codex-mini', 'high', null]]);
});

it('03 A.3.3, A.3.4: a level, a switch and a speed each change the choice; the model with no levels says so', async () => {
  const { picks } = show(CAN, true);
  const radio = (name: string): HTMLElement => {
    const found = [...document.querySelectorAll<HTMLElement>('[role="dialog"] [role="radio"]')].find((element) => element.textContent.trim().startsWith(name));
    if (found === undefined) throw new Error(`no radio ${name}`);
    return found;
  };
  radio('Low').click();
  flushSync();
  one('[role="switch"]').click();
  flushSync();
  radio('Fast').click();
  flushSync();
  expect(picks.map((pick) => [pick.effort, pick.toggles.join(), pick.speed])).toEqual([['low', 'thinking', 'standard'], ['low', '', 'standard'], ['low', '', 'fast']]);
  one('[role="toolbar"] [aria-label="Claude Code"]').click();
  flushSync();
  [...document.querySelectorAll<HTMLElement>('[role="option"]')].find((option) => option.textContent.includes('Haiku 5'))?.click();
  await settle();
  expect(document.body.textContent).toContain('No effort levels');
  expect(document.querySelector('[aria-label="Speed"]')).toBeNull();
});

it('03 A.3.4, A.4.2: the foot opens the Permissions section on the mode in use; a pick changes the mode and closes it', async () => {
  const { picks } = show(CAN, true);
  const button = one('[aria-label="Permissions: Supervised"]');
  button.click();
  await settle();
  const modes = [...document.querySelectorAll<HTMLElement>('[aria-label="Permissions"] [role="radio"]')];
  expect(modes.map((mode) => `${mode.getAttribute('aria-label') ?? ''}:${mode.getAttribute('aria-checked') ?? ''}`)).toEqual(['Supervised:true', 'Auto-accept edits:false', 'Auto:false', 'Full access:false']);
  expect(document.activeElement).toBe(modes[0]);
  key(modes[0] as HTMLElement, 'ArrowRight');
  flushSync();
  expect(document.activeElement).toBe(modes[1]);
  expect(texts('.shown .name, .shown .description:not(.hidden)')).toEqual(['Auto-accept edits', 'Auto-approve edits, ask before other actions.']);
  modes[3]?.click();
  await settle();
  expect(picks.map((pick) => pick.mode)).toEqual(['full-access']);
  expect([document.activeElement?.getAttribute('aria-label'), document.activeElement?.getAttribute('aria-expanded')]).toEqual(['Permissions: Full access', 'false']);
});

it('a picker that cannot be used keeps focus, says so, and does not open', async () => {
  const { trigger } = show(cannot('The models are loading.'));
  expect([trigger.getAttribute('aria-disabled'), trigger.disabled]).toEqual(['true', false]);
  trigger.click();
  await settle();
  expect(document.querySelector('[role="dialog"]')).toBeNull();
});

it('says why the list is short on a line of its own under the list, with the whole reason and a Retry when it failed', () => {
  let retried = 0;
  const words = 'Models could not load: the runner answered "spawn claude ENOENT" for the program at /usr/local/bin/claude.';
  const props = { catalogue: { ...pickerCatalogue, notice: { words, failed: true } }, value: pickerChoice, can: CAN, open: true, onChange: () => undefined, onAsk: () => (retried += 1) };
  app = mount(ModelPicker, { target: document.body, props });
  flushSync();
  const notice = one('.popup .notice');
  expect(notice.querySelector('.words')?.textContent).toBe(words);
  expect(notice.closest('[role="option"]')).toBeNull();
  notice.querySelector<HTMLButtonElement>('button')?.click();
  expect(retried).toBe(1);
});
