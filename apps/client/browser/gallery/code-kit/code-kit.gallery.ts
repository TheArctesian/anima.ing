import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { HUB_ORIGIN } from '../../synthetic/access';
import { installSyntheticHub } from '../../synthetic/synthetic-hub';
import { HEIGHT, MODES, storedSettings } from '../shots';

// The kit of the Code section (stage 1 of the T3 Code copy), each primitive in each state. It needs a build made with
// `VITE_ANIMA_SPECIMEN=1` and runs only when asked, `GALLERY_CODE_KIT=1`; pictures go to `gallery-shots/code-kit/<mode>/`.

const FOLDER = fileURLToPath(new URL('../../../gallery-shots/code-kit/', import.meta.url));
const SETTLE_MS = 300;
const MARGIN = 12;

interface KitShot {
  readonly name: string;
  /** The part to frame, found by its `data-kit` mark. */
  readonly part: string;
  /** Puts the part in the state to shoot; returns the floating layer that the picture must hold too. */
  readonly act?: (page: Page, part: Locator) => Promise<Locator | undefined>;
}

const hover = (name: string) => async (page: Page, part: Locator) => {
  await part.getByRole('button', { name }).hover();
  const tip = page.getByRole('tooltip');
  await expect(tip).toBeVisible();
  return tip;
};

const openMenu = (name: string, key?: string) => async (page: Page, part: Locator) => {
  const trigger = part.getByRole('button', { name });
  if (key === undefined) await trigger.click();
  else {
    await trigger.focus();
    await page.keyboard.press(key);
  }
  const menu = page.getByRole('menu');
  await expect(menu).toBeVisible();
  return menu;
};

const SHOTS: readonly KitShot[] = [
  { name: 'status', part: 'status' },
  { name: 'kbd', part: 'kbd' },
  { name: 'tooltip-rest', part: 'tooltip' },
  { name: 'tooltip-hover', part: 'tooltip', act: hover('Settle') },
  { name: 'tooltip-icon-button', part: 'tooltip', act: hover('Snooze') },
  { name: 'menu-rest', part: 'menu' },
  { name: 'menu-open-below', part: 'menu', act: openMenu('Chat actions') },
  { name: 'menu-open-above-end', part: 'menu', act: openMenu('More options') },
  {
    name: 'menu-keyboard-skips-disabled',
    part: 'menu',
    act: async (page, part) => {
      const menu = await openMenu('Chat actions', 'ArrowDown')(page, part);
      await page.keyboard.press('ArrowDown');
      await page.keyboard.press('ArrowDown');
      return menu;
    },
  },
  {
    name: 'scroll-both-fades',
    part: 'scroll',
    act: async (_page, part) => {
      const areas = part.getByRole('region');
      for (const area of await areas.all()) await area.evaluate((node) => node.scrollTo({ top: 120 }));
      await areas.first().hover();
      return undefined;
    },
  },
  { name: 'approval-command', part: 'approval-command' },
  { name: 'approval-file-change', part: 'approval-file-change' },
  { name: 'approval-file-read-reason', part: 'approval-file-read' },
  { name: 'approval-permission-app', part: 'approval-permission' },
  { name: 'approval-app-access', part: 'approval-app-access' },
  { name: 'approval-long-detail', part: 'approval-long' },
  { name: 'approval-more-open', part: 'approval-command', act: openMenu('More approval options') },
  {
    name: 'approval-approve-focus',
    part: 'approval-command',
    act: async (page, part) => {
      await part.getByRole('button', { name: 'Decline' }).focus();
      await page.keyboard.press('Tab');
      return undefined;
    },
  },
  { name: 'approval-locked', part: 'approval-locked' },
  { name: 'approval-process-gone', part: 'approval-gone' },
];

/** The smallest box that holds the part and its floating layer, with a margin, inside the window. */
async function frame(page: Page, part: Locator, layer: Locator | undefined): Promise<{ x: number; y: number; width: number; height: number }> {
  const boxes = [await part.boundingBox(), layer === undefined ? null : await layer.boundingBox()].filter((box) => box !== null);
  const left = Math.max(0, Math.min(...boxes.map((box) => box.x)) - MARGIN);
  const top = Math.max(0, Math.min(...boxes.map((box) => box.y)) - MARGIN);
  const size = page.viewportSize() ?? { width: 1280, height: HEIGHT };
  const right = Math.min(size.width, Math.max(...boxes.map((box) => box.x + box.width)) + MARGIN);
  const bottom = Math.min(size.height, Math.max(...boxes.map((box) => box.y + box.height)) + MARGIN);
  return { x: left, y: top, width: right - left, height: bottom - top };
}

for (const chosen of MODES.filter((mode) => !mode.dyslexic && mode.width === 1280)) {
  test.describe(`code-kit-${chosen.folder}`, () => {
    test.skip(process.env['GALLERY_CODE_KIT'] !== '1', 'The pictures of the Code kit are made only when asked.');
    test.use({ viewport: { width: chosen.width, height: HEIGHT }, colorScheme: chosen.theme });

    test.beforeEach(async ({ page, context }) => {
      mkdirSync(`${FOLDER}${chosen.folder}`, { recursive: true });
      await context.addInitScript((settings) => {
        for (const [key, value] of Object.entries(settings)) localStorage.setItem(key, value);
      }, storedSettings(chosen));
      const hub = await installSyntheticHub(context, {});
      await hub.signIn();
      await page.goto(`${HUB_ORIGIN}/code-kit`);
      await expect(page.getByRole('heading', { level: 1, name: 'Code kit' })).toBeVisible();
      await page.evaluate(() => document.fonts.ready);
    });

    test('page', async ({ page }) => {
      await page.waitForTimeout(SETTLE_MS);
      await page.screenshot({ path: `${FOLDER}${chosen.folder}/00-page.png`, fullPage: true });
    });

    SHOTS.forEach((shot, index) => {
      test(shot.name, async ({ page }) => {
        const part = page.locator(`[data-kit="${shot.part}"]`);
        await part.scrollIntoViewIfNeeded();
        await page.evaluate(() => window.scrollBy(0, -160));
        const layer = await shot.act?.(page, part);
        await page.waitForTimeout(SETTLE_MS);
        const file = `${String(index + 1).padStart(2, '0')}-${shot.name}.png`;
        await page.screenshot({ path: `${FOLDER}${chosen.folder}/${file}`, clip: await frame(page, part, layer) });
      });
    });
  });
}
