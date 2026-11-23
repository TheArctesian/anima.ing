import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { HUB_ORIGIN } from '../../synthetic/access';
import { installSyntheticHub } from '../../synthetic/synthetic-hub';
import { HEIGHT, MODES, storedSettings } from '../shots';
import { installDesktopShell } from '../threads/desktop-shell';
import { EVERY_STATE } from '../threads/scenario';

// The frame of Hosting (HSE-001, HSE-010) in the desktop application, so the bar has Code above Hosting: each section
// that the frame draws, and Settings with Routes gone from its page and kept in its index. `GALLERY_CODE_SHELL=1`;
// pictures go to `gallery-shots/code-shell/<mode>/hosting-*.png`.

const FOLDER = fileURLToPath(new URL('../../../gallery-shots/code-shell/', import.meta.url));
const SHOTS = [
  { name: 'hosting-1-applications', path: '/hosting', heading: 'Applications are not listed here yet' },
  { name: 'hosting-2-catalogue', path: '/hosting/catalogue', heading: 'Catalogue' },
  { name: 'hosting-3-routes', path: '/hosting/routes', heading: 'Routes' },
  { name: 'hosting-4-machines', path: '/hosting/machines', heading: 'Machines are not listed here yet' },
] as const;

async function opened(page: Page, path: string, settings: Record<string, string>): Promise<void> {
  await page.context().addInitScript((stored) => {
    for (const [key, value] of Object.entries(stored)) localStorage.setItem(key, value);
  }, settings);
  await installDesktopShell(page.context(), EVERY_STATE);
  const hub = await installSyntheticHub(page.context(), {});
  await hub.signIn();
  await page.goto(`${HUB_ORIGIN}${path}`);
}

async function shot(page: Page, path: string): Promise<void> {
  await page.mouse.move(5, HEIGHT - 5);
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(400);
  await page.screenshot({ path });
}

for (const chosen of MODES.filter((mode) => !mode.dyslexic && mode.width === 1280)) {
  test.describe(`hosting-${chosen.folder}`, () => {
    test.skip(process.env['GALLERY_CODE_SHELL'] !== '1', 'The pictures of the Code page are made only when asked.');
    test.use({ viewport: { width: chosen.width, height: HEIGHT }, colorScheme: chosen.theme });
    test.beforeAll(() => mkdirSync(`${FOLDER}${chosen.folder}`, { recursive: true }));

    for (const each of SHOTS) {
      test(`${each.name}.png`, async ({ page }) => {
        await opened(page, each.path, storedSettings(chosen));
        await expect(page.getByRole('navigation', { name: 'Hosting sections' })).toBeVisible();
        await expect(page.getByRole('heading', { name: each.heading }).first()).toBeVisible();
        if (each.path === '/hosting/routes') await expect(page.locator('main header').getByRole('button', { name: 'Add a route' })).toBeVisible();
        await shot(page, `${FOLDER}${chosen.folder}/${each.name}.png`);
      });
    }

    test('hosting-5-settings-index.png', async ({ page }) => {
      await opened(page, '/settings#routes', storedSettings(chosen));
      await expect(page).toHaveURL(/\/hosting\/routes$/);
      await page.goto(`${HUB_ORIGIN}/settings`);
      await expect(page.getByRole('link', { name: 'Routes in Hosting' })).toHaveAttribute('href', '/hosting/routes');
      await shot(page, `${FOLDER}${chosen.folder}/hosting-5-settings-index.png`);
    });
  });
}

// At 1440 px the section takes the page's width, so its content ends where the action of the header ends.
for (const chosen of MODES.filter((mode) => !mode.dyslexic && mode.width === 1280)) {
  const folder = chosen.folder.replace('1280', '1440');
  test.describe(`hosting-wide-${folder}`, () => {
    test.skip(process.env['GALLERY_CODE_SHELL'] !== '1', 'The pictures of the Code page are made only when asked.');
    test.use({ viewport: { width: 1440, height: HEIGHT }, colorScheme: chosen.theme });
    test.beforeAll(() => mkdirSync(`${FOLDER}${folder}`, { recursive: true }));

    test('hosting-6-routes-wide.png', async ({ page }) => {
      await opened(page, '/hosting/routes', storedSettings(chosen));
      const action = page.locator('main header').getByRole('button', { name: 'Add a route' });
      await expect(action).toBeVisible();
      const [end, edge] = await Promise.all([action.evaluate((each) => each.getBoundingClientRect().right), page.getByRole('region', { name: 'Routes' }).evaluate((each) => each.getBoundingClientRect().right)]);
      expect(Math.abs(end - edge)).toBeLessThan(2);
      await shot(page, `${FOLDER}${folder}/hosting-6-routes-wide.png`);
    });
  });
}
