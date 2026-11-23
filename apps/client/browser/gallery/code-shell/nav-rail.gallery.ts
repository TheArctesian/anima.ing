import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { HUB_ORIGIN } from '../../synthetic/access';
import { installSyntheticHub } from '../../synthetic/synthetic-hub';
import { HEIGHT, MODES, storedSettings } from '../shots';
import { installDesktopShell } from '../threads/desktop-shell';
import { EVERY_STATE } from '../threads/scenario';

// The collapsed bar of Anima, collapsed by its button on Devices and folded by the page Code, side by side: one rail
// (operator, 2026-11-22). The gallery's WebKit reports a Mac, so the rail keeps room for the window buttons.
// `GALLERY_CODE_SHELL=1`; pictures go to `gallery-shots/code-shell/<theme>-1280/20-nav-rail.png`.

const FOLDER = fileURLToPath(new URL('../../../gallery-shots/code-shell/', import.meta.url));
const SETTLE_MS = 400;
const RAIL = { x: 0, y: 0, width: 360, height: HEIGHT };

async function railOf(page: Page): Promise<string> {
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(SETTLE_MS);
  return (await page.screenshot({ clip: RAIL })).toString('base64');
}

for (const chosen of MODES.filter((mode) => !mode.dyslexic && mode.width === 1280)) {
  test.describe(`nav-rail-${chosen.folder}`, () => {
    test.skip(process.env['GALLERY_CODE_SHELL'] !== '1', 'The pictures of the rail are made only when asked.');
    test.use({ viewport: { width: chosen.width, height: HEIGHT }, colorScheme: chosen.theme });
    test('20-nav-rail.png', async ({ page }) => {
      mkdirSync(`${FOLDER}${chosen.folder}`, { recursive: true });
      await page.context().addInitScript((settings) => {
        for (const [key, value] of Object.entries(settings)) localStorage.setItem(key, value);
      }, storedSettings(chosen));
      await installDesktopShell(page.context(), EVERY_STATE);
      const hub = await installSyntheticHub(page.context(), {});
      await hub.signIn();
      await page.goto(`${HUB_ORIGIN}/devices`);
      await page.getByRole('button', { name: 'Collapse sidebar' }).click();
      await expect(page.getByRole('button', { name: 'Expand sidebar' })).toBeVisible();
      const byButton = await railOf(page);
      await page.evaluate(() => localStorage.setItem('anima.nav.collapsed', 'false'));
      await page.goto(`${HUB_ORIGIN}/code`);
      await expect(page.getByRole('button', { name: 'Expand sidebar' })).toBeVisible();
      const byCode = await railOf(page);
      const panel = (title: string, png: string) =>
        `<figure><figcaption>${title}</figcaption><img src="data:image/png;base64,${png}"></figure>`;
      await page.setContent(
        `<style>body{margin:0;display:flex;gap:24px;padding:16px;background:${chosen.theme === 'dark' ? '#2e3440' : '#eceff4'};` +
          `font:14px sans-serif;color:${chosen.theme === 'dark' ? '#eceff4' : '#2e3440'}}figure{margin:0}img{display:block;border:1px solid #888}</style>` +
          panel('Collapsed by its button (Devices)', byButton) +
          panel('Folded by Code', byCode),
      );
      await page.screenshot({ path: `${FOLDER}${chosen.folder}/20-nav-rail.png`, fullPage: true });
    });
  });
}
