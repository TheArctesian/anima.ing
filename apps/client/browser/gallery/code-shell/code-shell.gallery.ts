import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { HUB_ORIGIN } from '../../synthetic/access';
import { installSyntheticHub } from '../../synthetic/synthetic-hub';
import { HEIGHT, MODES, storedSettings, type Mode } from '../shots';
import { installDesktopShell, type Scenario } from '../threads/desktop-shell';
import { EVERY_STATE as UNMARKED, IDS } from '../threads/scenario';

// Stage 2 of the T3 Code copy (TC2): the inbox of the page Code in each state, and the whole page. The page is only in
// the desktop application, so the gallery stands in for its shell with the made-up threads of the threads gallery. It
// runs only when asked, `GALLERY_CODE_SHELL=1`; pictures go to `gallery-shots/code-shell/<mode>/`.

const FOLDER = fileURLToPath(new URL('../../../gallery-shots/code-shell/', import.meta.url));
const SETTLE_MS = 400;
const MARGIN = 12;
/**
 * The threads of the threads gallery with the marks the runner keeps (TH61): one settled by hand, the pinned "Port the
 * battery meter" snoozed for two hours (a snooze takes a thread out of the pinned group), and one woken by hand.
 */
const MARKS: Readonly<Record<string, Record<string, string>>> = {
  [IDS.earlier]: { settled_at: '@-100' },
  [IDS.pinnedFile]: { snoozed_at: '@-30', snoozed_until: '@120' },
  [IDS.yourTurn]: { woke_at: '@-2' },
};
const EVERY_STATE: Scenario = {
  ...UNMARKED,
  listings: [
    ...UNMARKED.listings.map((listing) => ({ ...listing, ...MARKS[String(listing['thread'])] })),
    ...Object.entries(MARKS)
      .filter(([thread]) => !UNMARKED.listings.some((listing) => listing['thread'] === thread))
      .map(([thread, marks]) => ({ thread, pinned: false, archived: false, title: null, removed: false, ...marks })),
  ],
};
const EMPTY: Scenario = { ...EVERY_STATE, rows: [], running: [], listings: [], updates: [] };
const TALKED = 'Why does the gates run stop at limits?';
/** The idle thread works in a linked worktree on a branch, with the context window its program last said (TH112). */
const DETAILED: Scenario = {
  ...EVERY_STATE,
  running: EVERY_STATE.running.map((row) =>
    row['title'] === TALKED
      ? { ...row, workspace: { kind: 'worktree', top: '/Users/you/anima', branch: 'code/gates-limits' }, usage: { context: 48_200, window: 200_000, input: 1_200, output: 3_400, cost_usd: 0.42 } }
      : row,
  ),
};

/**
 * Opens the thread with details. In a 1440 px window the card stands inline beside the lane; at 1280 px, beside Anima's
 * bar and the list, the canvas is under T3's 1012 px, so the card opens as a popover from the toggle.
 */
async function openDetails(page: Page, inbox: Locator, popover: boolean): Promise<readonly Locator[]> {
  if (!popover) await page.setViewportSize({ width: 1440, height: HEIGHT });
  // The context window is off by default, as in T3; the person turns it on from the card's menu.
  await page.evaluate(() => localStorage.setItem('anima.code.details-items', JSON.stringify({ context: true })));
  await card(inbox, TALKED).click();
  if (popover) {
    await page.getByRole('button', { name: 'Toggle thread details' }).click();
    // The pointer leaves the toggle after the click, so its tooltip does not cover the popover.
    await page.mouse.move(700, HEIGHT / 2);
  }
  await expect(page.getByRole('complementary', { name: 'Thread details' })).toBeVisible();
  return [];
}

interface Shot {
  readonly name: string;
  readonly scenario?: Scenario;
  /** The whole window, not the inbox column. */
  readonly window?: true;
  /** Puts the page in the state to shoot; returns the floating layers the picture must hold too. */
  readonly act?: (page: Page, inbox: Locator) => Promise<readonly Locator[]>;
}

const card = (inbox: Locator, title: string): Locator => inbox.getByRole('button', { name: new RegExp(title) }).first();

async function openCode(page: Page): Promise<Locator> {
  await page.goto(`${HUB_ORIGIN}/code`);
  const inbox = page.getByRole('complementary', { name: 'Threads of this computer' });
  await expect(inbox.getByLabel('Search threads')).toBeVisible();
  return inbox;
}

async function shelf(inbox: Locator, name: string): Promise<void> {
  await inbox.getByRole('button', { name: new RegExp(`^${name}`) }).click();
}

const SHOTS: readonly Shot[] = [
  { name: 'inbox', act: async (_page, inbox) => (await expect(card(inbox, 'Review the sign-in flow')).toBeVisible(), []) },
  {
    name: 'row-hover-actions',
    act: async (page, inbox) => {
      await card(inbox, 'Review the sign-in flow').hover();
      await inbox.locator('li').filter({ hasText: 'Review the sign-in flow' }).getByRole('button', { name: 'Settle thread' }).hover();
      const tip = page.getByRole('tooltip');
      await expect(tip).toBeVisible();
      return [tip];
    },
  },
  {
    name: 'shelves-open',
    act: async (_page, inbox) => {
      await shelf(inbox, 'Working');
      await shelf(inbox, 'Snoozed');
      await shelf(inbox, 'Settled');
      await inbox.getByRole('region', { name: 'Threads' }).evaluate((node) => node.scrollTo({ top: node.scrollHeight }));
      await expect(card(inbox, 'Tidy the device page')).toBeInViewport();
      return [];
    },
  },
  {
    name: 'row-open',
    act: async (page, inbox) => {
      await card(inbox, 'Review the sign-in flow').click();
      await expect(page.getByRole('region', { name: 'Thread' }).or(page.getByLabel('Thread')).first()).toBeVisible();
      return [];
    },
  },
  {
    name: 'row-menu',
    act: async (page, inbox) => {
      await card(inbox, 'Bump the Rust toolchain').click({ button: 'right' });
      const menu = page.getByRole('menu', { name: 'Thread actions' });
      await expect(menu).toBeVisible();
      return [menu];
    },
  },
  {
    name: 'multi-select-menu',
    act: async (page, inbox) => {
      const mod = process.platform === 'darwin' ? 'Meta' : 'Control';
      await card(inbox, 'Review the sign-in flow').click({ modifiers: [mod] });
      await card(inbox, 'Bump the Rust toolchain').click({ modifiers: [mod] });
      await card(inbox, 'Bump the Rust toolchain').click({ button: 'right' });
      const menu = page.getByRole('menu', { name: '2 threads' });
      await expect(menu).toBeVisible();
      return [menu];
    },
  },
  {
    name: 'rename',
    act: async (page, inbox) => {
      await card(inbox, 'Review the sign-in flow').focus();
      await page.keyboard.press('F2');
      await expect(inbox.getByLabel('Thread title')).toBeFocused();
      return [];
    },
  },
  {
    name: 'project-filter',
    act: async (page, inbox) => {
      await inbox.getByRole('button', { name: /Filter threads by project/ }).click();
      const panel = page.getByRole('listbox', { name: 'Projects' });
      await expect(panel).toBeVisible();
      return [panel.locator('..')];
    },
  },
  {
    name: 'project-filtered',
    act: async (page, inbox) => {
      await inbox.getByRole('button', { name: /Filter threads by project/ }).click();
      await page.getByRole('option', { name: /notes/ }).click();
      // Only the running thread of "notes" is left: it is on the Working shelf, which T3 counts, so the list is not empty.
      await expect(inbox.getByRole('button', { name: 'Working 1' })).toBeVisible();
      return [];
    },
  },
  {
    name: 'search',
    act: async (page, inbox) => {
      await inbox.getByLabel('Search threads').fill('the');
      await page.keyboard.press('ArrowDown');
      await expect(inbox.getByRole('listbox', { name: 'Search results' })).toBeVisible();
      return [];
    },
  },
  {
    name: 'search-none',
    act: async (_page, inbox) => {
      await inbox.getByLabel('Search threads').fill('zzz');
      await expect(inbox.getByText('No threads found')).toBeVisible();
      return [];
    },
  },
  { name: 'empty', scenario: EMPTY, act: async (_page, inbox) => (await expect(inbox.getByText('No threads yet')).toBeVisible(), []) },
  {
    name: 'inbox-hidden',
    window: true,
    act: async (page) => {
      // T3 Code keeps the toggle in the corner of the page, outside the column it hides.
      await page.getByRole('button', { name: /^Toggle the thread list/ }).click();
      await expect(page.getByRole('button', { name: /^Toggle the thread list/ })).toHaveAttribute('aria-pressed', 'false');
      return [];
    },
  },
  { name: 'details-inline', window: true, scenario: DETAILED, act: (page, inbox) => openDetails(page, inbox, false) },
  { name: 'details-popover', window: true, scenario: DETAILED, act: (page, inbox) => openDetails(page, inbox, true) },
];

/** The box of the inbox column with its floating layers, in the page. */
async function frame(inbox: Locator, layers: readonly Locator[]): Promise<{ x: number; y: number; width: number; height: number }> {
  const boxes = (await Promise.all([inbox, ...layers].map((each) => each.boundingBox()))).filter((box) => box !== null);
  const left = Math.max(0, Math.min(...boxes.map((box) => box.x)) - MARGIN);
  const top = Math.max(0, Math.min(...boxes.map((box) => box.y)) - MARGIN);
  const right = Math.max(...boxes.map((box) => box.x + box.width)) + MARGIN;
  const bottom = Math.max(...boxes.map((box) => box.y + box.height)) + MARGIN;
  return { x: left, y: top, width: right - left, height: bottom - top };
}

async function prepare(page: Page, chosen: Mode, scenario: Scenario): Promise<Locator> {
  await page.context().addInitScript((settings) => {
    for (const [key, value] of Object.entries(settings)) localStorage.setItem(key, value);
  }, storedSettings(chosen));
  await installDesktopShell(page.context(), scenario);
  const hub = await installSyntheticHub(page.context(), {});
  await hub.signIn();
  return openCode(page);
}

const run = process.env['GALLERY_CODE_SHELL'] === '1';
const numbered = SHOTS.map((shot, index) => ({ shot, file: `${String(index + 1).padStart(2, '0')}-${shot.name}.png` }));

for (const chosen of MODES.filter((mode) => !mode.dyslexic && mode.width === 1280)) {
  test.describe(`code-shell-${chosen.folder}`, () => {
    test.skip(!run, 'The pictures of the Code inbox are made only when asked.');
    test.use({ viewport: { width: chosen.width, height: HEIGHT }, colorScheme: chosen.theme });
    for (const { shot, file } of numbered) {
      test(file, async ({ page }) => {
        mkdirSync(`${FOLDER}${chosen.folder}`, { recursive: true });
        const inbox = await prepare(page, chosen, shot.scenario ?? EVERY_STATE);
        const layers = (await shot.act?.(page, inbox)) ?? [];
        await page.evaluate(() => document.fonts.ready);
        await page.waitForTimeout(SETTLE_MS);
        const path = `${FOLDER}${chosen.folder}/${file}`;
        await page.screenshot(shot.window === true ? { path } : { path, clip: await frame(inbox, layers) });
      });
    }
  });
}

// The whole page Code at 1440 wide, a thread open, the bar of Anima folded to its icons (OD-THR60).
for (const theme of ['dark', 'light'] as const) {
  const base = MODES.find((mode) => mode.theme === theme && !mode.dyslexic);
  if (base === undefined) continue;
  const wide: Mode = { ...base, width: 1440, folder: `${theme}-1440` };
  test.describe(`code-shell-${wide.folder}`, () => {
    test.skip(!run, 'The pictures of the Code page are made only when asked.');
    test.use({ viewport: { width: 1440, height: HEIGHT }, colorScheme: theme });
    test('00-page.png', async ({ page }) => {
      mkdirSync(`${FOLDER}${wide.folder}`, { recursive: true });
      const inbox = await prepare(page, wide, EVERY_STATE);
      await card(inbox, 'Why does the gates run stop at limits').click();
      await expect(page.getByText('So the way out of a failed limit')).toBeVisible();
      await page.evaluate(() => document.fonts.ready);
      await page.waitForTimeout(SETTLE_MS);
      await page.screenshot({ path: `${FOLDER}${wide.folder}/00-page.png` });
    });

    // The lists of the composer on the real page: they hang over the card, above the timeline (TC4, "Popup design").
    // The engine gives the composer no files yet, so "@" shows the empty list; "/" lists the built-in commands.
    for (const [name, text, shows] of [['14-mention-list.png', 'Read @', 'The runner cannot search the folder of a chat yet.'], ['15-command-list.png', '/', null]] as const) {
      test(name, async ({ page }) => {
        mkdirSync(`${FOLDER}${wide.folder}`, { recursive: true });
        const inbox = await prepare(page, wide, EVERY_STATE);
        await card(inbox, 'Why does the gates run stop at limits').click();
        await page.getByRole('textbox', { name: /./ }).last().click();
        await page.keyboard.type(text);
        await expect(shows === null ? page.getByRole('listbox').last() : page.getByRole('paragraph').filter({ hasText: shows })).toBeVisible();
        await page.evaluate(() => document.fonts.ready);
        await page.waitForTimeout(SETTLE_MS);
        await page.screenshot({ path: `${FOLDER}${wide.folder}/${name}` });
      });
    }
  });
}
