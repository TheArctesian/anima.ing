import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { HUB_ORIGIN } from '../../synthetic/access';
import { installSyntheticHub } from '../../synthetic/synthetic-hub';
import { HEIGHT, MODES, storedSettings } from '../shots';

// The timeline of the Code section (stage 3 of the T3 Code copy), each state of a chat. It needs a build made with
// `VITE_ANIMA_SPECIMEN=1` and runs only when asked, `GALLERY_CODE_TIMELINE=1`; pictures go to
// `gallery-shots/code-timeline/<mode>/`.

const FOLDER = fileURLToPath(new URL('../../../gallery-shots/code-timeline/', import.meta.url));
const SETTLE_MS = 300;

interface TimelineShot {
  readonly name: string;
  /** The scene to frame, by its `data-kit` mark without `timeline-`. */
  readonly scene: string;
  /** Puts the scene in the state to shoot. */
  readonly act?: (page: Page, scene: Locator) => Promise<void>;
}

const click = (name: string | RegExp) => async (_page: Page, scene: Locator) => {
  await scene.getByRole('button', { name }).first().click();
};

const SHOTS: readonly TimelineShot[] = [
  { name: 'finished-folded', scene: 'finished' },
  { name: 'finished-fold-open', scene: 'finished', act: click(/^Worked for/) },
  {
    name: 'finished-group-open',
    scene: 'finished',
    act: async (page, scene) => {
      await click(/^Worked for/)(page, scene);
      await scene.getByRole('button', { name: /^Read 1 file/ }).click();
    },
  },
  {
    name: 'finished-row-detail',
    scene: 'finished',
    act: async (page, scene) => {
      await click(/^Worked for/)(page, scene);
      await scene.getByRole('button', { name: /^Read 1 file/ }).click();
      await scene.getByRole('button', { name: 'git status --short' }).click();
    },
  },
  { name: 'code-block-wrap', scene: 'finished', act: click('Wrap lines') },
  {
    name: 'user-actions-hover',
    scene: 'finished',
    act: async (_page, scene) => {
      await scene.locator('.bubble').first().hover();
    },
  },
  { name: 'working-live-line', scene: 'working' },
  { name: 'working-live-group-open', scene: 'working', act: click(/^Running bun/) },
  { name: 'approval-inline', scene: 'approval' },
  {
    name: 'approval-more-open',
    scene: 'approval',
    act: async (page, scene) => {
      await scene.getByRole('button', { name: 'More approval options' }).click();
      await expect(page.getByRole('menu')).toBeVisible();
    },
  },
  { name: 'question-line', scene: 'question' },
  { name: 'stopped-and-steer', scene: 'stopped' },
  { name: 'stopped-fold-open', scene: 'stopped', act: click(/^You stopped after/) },
  { name: 'failed-command', scene: 'failed', act: click(/^Worked for/) },
  { name: 'long-message-folded', scene: 'long' },
  { name: 'long-message-open', scene: 'long', act: click('Show full message') },
  {
    name: 'scroll-to-end',
    scene: 'long',
    act: async (_page, scene) => {
      await click('Show full message')(_page, scene);
      // The timeline follows the end as the message opens; scroll up only once it rests. Only a person's scroll (the
      // wheel here) releases the follow, so the shot scrolls with the wheel and not with `scrollTo`.
      await _page.waitForTimeout(SETTLE_MS);
      await scene.locator('.scroller').hover();
      await _page.mouse.wheel(0, -4000);
      await expect(scene.getByRole('button', { name: 'Scroll to end' })).toBeVisible();
    },
  },
  { name: 'empty', scene: 'empty' },
];

for (const chosen of MODES.filter((mode) => !mode.dyslexic && mode.width === 1280)) {
  test.describe(`code-timeline-${chosen.folder}`, () => {
    test.skip(process.env['GALLERY_CODE_TIMELINE'] !== '1', 'The pictures of the Code timeline are made only when asked.');
    test.use({ viewport: { width: chosen.width, height: HEIGHT }, colorScheme: chosen.theme });

    test.beforeEach(async ({ page, context }) => {
      mkdirSync(`${FOLDER}${chosen.folder}`, { recursive: true });
      await context.addInitScript((settings) => {
        for (const [key, value] of Object.entries(settings)) localStorage.setItem(key, value);
      }, storedSettings(chosen));
      const hub = await installSyntheticHub(context, {});
      await hub.signIn();
      await page.goto(`${HUB_ORIGIN}/code-kit/timeline`);
      await expect(page.getByRole('heading', { level: 1, name: 'Code kit: timeline' })).toBeVisible();
      await page.evaluate(() => document.fonts.ready);
    });

    SHOTS.forEach((shot, index) => {
      test(shot.name, async ({ page }) => {
        const scene = page.locator(`[data-kit="timeline-${shot.scene}"]`);
        await scene.evaluate((node) => {
          node.scrollIntoView({ block: 'center' });
        });
        await shot.act?.(page, scene);
        await page.waitForTimeout(SETTLE_MS);
        const file = `${String(index + 1).padStart(2, '0')}-${shot.name}.png`;
        const box = await scene.boundingBox();
        if (box === null) throw new Error(`the scene ${shot.scene} is not on the page`);
        const menu = page.getByRole('menu');
        const layer = (await menu.count()) > 0 ? await menu.boundingBox() : null;
        const top = Math.max(0, Math.min(box.y, layer?.y ?? box.y));
        await page.screenshot({ path: `${FOLDER}${chosen.folder}/${file}`, clip: { x: box.x, y: top, width: box.width, height: box.y + box.height - top } });
      });
    });
  });
}
