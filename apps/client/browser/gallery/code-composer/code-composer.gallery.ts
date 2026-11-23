import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { HUB_ORIGIN } from '../../synthetic/access';
import { installSyntheticHub } from '../../synthetic/synthetic-hub';
import { HEIGHT, MODES, storedSettings } from '../shots';

// TC4. Needs a build with `VITE_ANIMA_SPECIMEN=1` and runs only with `GALLERY_CODE_COMPOSER=1`.

const FOLDER = fileURLToPath(new URL('../../../gallery-shots/code-composer/', import.meta.url));
const SETTLE_MS = 300;
const MARGIN = 12;

interface ComposerShot {
  readonly name: string;
  /** The `data-kit` mark of the part to frame. */
  readonly part: string;
  /** Returns the floating layer that the picture must hold too. */
  readonly act?: (page: Page, part: Locator) => Promise<Locator | undefined>;
}

const editorOf = (part: Locator): Locator => part.getByRole('textbox');

const hover = (name: string) => async (page: Page, part: Locator) => {
  await part.getByRole('button', { name }).hover();
  const tip = page.getByRole('tooltip');
  await expect(tip).toBeVisible();
  return tip;
};

const press = (name: string) => async (_page: Page, part: Locator) => {
  await part.getByRole('button', { name }).first().click();
  return undefined;
};

const typed = (text: string) => async (page: Page, part: Locator) => {
  await editorOf(part).click();
  await page.keyboard.type(text);
  const list = page.getByRole('listbox').first();
  await expect(list).toBeVisible();
  return list;
};

const SHOTS: readonly ComposerShot[] = [
  { name: 'card-rest', part: 'rest' },
  { name: 'card-focus-ring', part: 'rest', act: async (_page, part) => {
    await editorOf(part).click();
    return undefined;
  } },
  { name: 'card-rich-text', part: 'rich' },
  { name: 'card-rich-markers-at-caret', part: 'rich', act: async (page, part) => {
    await editorOf(part).click();
    await page.keyboard.press('ControlOrMeta+ArrowUp');
    await page.keyboard.press('ArrowDown');
    for (let step = 0; step < 8; step += 1) await page.keyboard.press('ArrowRight');
    return undefined;
  } },
  { name: 'card-send-tip', part: 'rich', act: hover('Submit message') },
  { name: 'card-send-failed', part: 'failed', act: press('Submit message') },
  { name: 'card-attach-tip', part: 'rest', act: hover('Attach files') },
  { name: 'running-stop', part: 'running-empty' },
  { name: 'running-stop-pending', part: 'running-empty', act: press('Stop generation') },
  { name: 'running-steer', part: 'running-steer' },
  { name: 'running-steer-tip', part: 'running-steer', act: hover('Steer message') },
  { name: 'queue-three', part: 'queue-three' },
  { name: 'queue-folded', part: 'queue-folded' },
  { name: 'queue-all', part: 'queue-folded', act: press('Show all 4 queued') },
  { name: 'queue-editing', part: 'queue-three', act: press('Edit queued message') },
  { name: 'attachments', part: 'attachments' },
  { name: 'mention-files', part: 'menu-files' },
  { name: 'mention-commands', part: 'menu-commands' },
  { name: 'mention-skills', part: 'menu-skills' },
  { name: 'mention-empty', part: 'menu-empty' },
  { name: 'mention-over-card', part: 'rest', act: typed('Read @comp') },
  { name: 'command-over-card', part: 'rest', act: typed('/') },
  { name: 'picker-closed', part: 'picker' },
  { name: 'picker-open', part: 'picker', act: async (page, part) => {
    await part.getByRole('button').first().click();
    const popup = page.getByRole('dialog');
    await expect(popup).toBeVisible();
    return popup;
  } },
  { name: 'voice-pills', part: 'voice' },
  { name: 'runner-idle', part: 'real-idle' },
  { name: 'runner-no-attachments-tip', part: 'real-idle', act: hover('Attach files') },
  { name: 'runner-fixed-model-tip', part: 'real-idle', act: async (page, part) => {
    await part.getByRole('button').nth(1).hover();
    const tip = page.getByRole('tooltip');
    await expect(tip).toBeVisible();
    return tip;
  } },
  { name: 'runner-running', part: 'real-running' },
  { name: 'runner-ended', part: 'real-ended' },
  { name: 'runner-read-only', part: 'real-read' },
  // At the end, so the numbers of the pictures before them stay.
  { name: 'queue-waits', part: 'queue-waits' },
  { name: 'runner-queued', part: 'real-running', act: async (page, part) => {
    await editorOf(part).click();
    await page.keyboard.type('Then run the gates of the changed paths.');
    await page.keyboard.press('ControlOrMeta+Enter');
    await expect(part.getByRole('listitem')).toHaveCount(1);
    return undefined;
  } },
  { name: 'picker-full-access', part: 'picker', act: async (page, part) => {
    await part.getByRole('button').first().click();
    await page.getByRole('dialog').first().getByRole('button', { name: /^Permissions: / }).click();
    const menu = page.getByRole('dialog', { name: 'Permissions' });
    await menu.getByRole('radio', { name: 'Full access' }).hover();
    await expect(menu.getByText(/checkpoints/u).last()).toBeVisible();
    const tall = (await menu.boundingBox())?.height;
    await menu.getByRole('radio', { name: 'Supervised' }).hover();
    expect((await menu.boundingBox())?.height).toBe(tall);
    await menu.getByRole('radio', { name: 'Full access' }).hover();
    return page.getByRole('dialog').first();
  } },
  { name: 'runner-no-search', part: 'real-idle', act: async (page, part) => {
    await editorOf(part).click();
    await page.keyboard.type('Read @gate');
    const words = page.getByRole('paragraph').filter({ hasText: 'The runner cannot search the folder of a chat yet.' });
    await expect(words).toBeVisible();
    return words;
  } },
  { name: 'runner-no-commands', part: 'real-idle', act: async (page, part) => {
    await editorOf(part).click();
    await page.keyboard.type('/');
    const words = page.getByRole('paragraph').filter({ hasText: 'No matching command.' });
    await expect(words).toBeVisible();
    return words;
  } },
  { name: 'runner-commands', part: 'real-set', act: async (page, part) => {
    await editorOf(part).click();
    await page.keyboard.type('/');
    // The page's specimen menus come first; the one that this card opened is the last list of commands.
    const list = page.getByRole('listbox', { name: 'Commands' }).last();
    await expect(list.getByRole('option')).toHaveCount(1);
    return list;
  } },
];

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
  test.describe(`code-composer-${chosen.folder}`, () => {
    test.skip(process.env['GALLERY_CODE_COMPOSER'] !== '1', 'The pictures of the composer are made only when asked.');
    test.use({ viewport: { width: chosen.width, height: HEIGHT }, colorScheme: chosen.theme });

    test.beforeEach(async ({ page, context }) => {
      mkdirSync(`${FOLDER}${chosen.folder}`, { recursive: true });
      await context.addInitScript((settings) => {
        for (const [key, value] of Object.entries(settings)) localStorage.setItem(key, value);
      }, storedSettings(chosen));
      const hub = await installSyntheticHub(context, {});
      await hub.signIn();
      await page.goto(`${HUB_ORIGIN}/code-kit/composer`);
      await expect(page.getByRole('heading', { level: 1, name: 'Code kit: composer' })).toBeVisible();
      await page.evaluate(() => document.fonts.ready);
    });

    test('page', async ({ page }) => {
      await page.waitForTimeout(SETTLE_MS);
      await page.screenshot({ path: `${FOLDER}${chosen.folder}/00-page.png`, fullPage: true });
    });

    SHOTS.forEach((shot, index) => {
      test(shot.name, async ({ page }) => {
        const part = page.locator(`[data-kit="${shot.part}"]`);
        await part.evaluate((node) => {
          node.scrollIntoView({ block: 'center' });
        });
        const layer = await shot.act?.(page, part);
        await page.waitForTimeout(SETTLE_MS);
        const file = `${String(index + 1).padStart(2, '0')}-${shot.name}.png`;
        await page.screenshot({ path: `${FOLDER}${chosen.folder}/${file}`, clip: await frame(page, part, layer) });
      });
    });
  });
}
