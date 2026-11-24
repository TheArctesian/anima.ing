import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { test } from '@playwright/test';
import { HUB_ORIGIN } from '../../synthetic/access';
import { installSyntheticHub } from '../../synthetic/synthetic-hub';
import { openSettings, section } from '../steps';

// Tile pictures of the Extensions page (OD-EXT-NEW-pictures); ../README.md says how they become `static/extensions/`.

const FOLDER = fileURLToPath(new URL('../../../gallery-shots/pictures/', import.meta.url));
const WINDOW = { width: 1280, height: 800 };

const PICTURES = [
  {
    key: 'projects',
    shoot: async (page: import('@playwright/test').Page, path: string) => {
      await page.goto(`${HUB_ORIGIN}/projects`);
      await page.getByText('example/site').first().waitFor();
      await page.waitForLoadState('networkidle');
      await page.locator('main').screenshot({ path });
    },
  },
  {
    key: 'integrations',
    shoot: async (page: import('@playwright/test').Page, path: string) => {
      await openSettings(page);
      await section(page, 'Integrations').scrollIntoViewIfNeeded();
      const box = await section(page, 'Integrations').boundingBox();
      if (box === null) throw new Error('The section Integrations is not on the page.');
      const margin = 24;
      await page.screenshot({ path, clip: { x: box.x - margin, y: box.y - margin, width: box.width + margin * 2, height: box.height + margin * 2 } });
    },
  },
] as const;

for (const theme of ['dark', 'light'] as const) {
  test.describe(`pictures-${theme}`, () => {
    test.skip(process.env['GALLERY_PICTURES'] !== '1', 'The pictures of the extensions are made only when asked.');
    test.use({ viewport: WINDOW, colorScheme: theme });
    for (const picture of PICTURES) {
      test(`${picture.key}-${theme}`, async ({ page, context }) => {
        mkdirSync(FOLDER, { recursive: true });
        await context.addInitScript((chosen) => {
          localStorage.setItem('anima.theme', chosen);
        }, theme);
        const hub = await installSyntheticHub(context, { fixtures: (loaded) => ({ ...loaded, extensionsOn: ['projects'] }) });
        await hub.signIn();
        await picture.shoot(page, `${FOLDER}${picture.key}-${theme}.png`);
      });
    }
  });
}
