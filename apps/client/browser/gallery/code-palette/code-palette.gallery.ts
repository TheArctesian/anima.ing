import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { HUB_ORIGIN } from '../../synthetic/access';
import { installSyntheticHub } from '../../synthetic/synthetic-hub';
import { HEIGHT, MODES, storedSettings } from '../shots';

// The command palette and the key table of the Code section (stage 6 of the T3 Code copy), each state. It needs a build
// made with `VITE_ANIMA_SPECIMEN=1` and runs only when asked, `GALLERY_CODE_PALETTE=1`; pictures go to
// `gallery-shots/code-palette/<mode>/`.

const FOLDER = fileURLToPath(new URL('../../../gallery-shots/code-palette/', import.meta.url));
const SETTLE_MS = 300;

interface PaletteShot {
  readonly name: string;
  /** The radios of the page to choose first: "Loading", "One open". */
  readonly settings?: readonly string[];
  /** What to type into the open palette. */
  readonly typed?: string;
  /** Keys to press after typing. */
  readonly keys?: readonly string[];
  /** A part of the page to frame in place of the open palette. */
  readonly part?: string;
}

const SHOTS: readonly PaletteShot[] = [
  { name: 'root' },
  { name: 'search-chats-first', typed: 'release' },
  { name: 'search-actions-first', typed: 'new' },
  { name: 'actions-only', typed: '>' },
  { name: 'keyboard-moves', keys: ['ArrowDown', 'ArrowDown', 'ArrowDown', 'ArrowDown', 'ArrowDown', 'ArrowDown'] },
  { name: 'keyboard-wraps-to-last', keys: ['ArrowUp'] },
  { name: 'chat-open', settings: ['One open'] },
  { name: 'no-result', typed: 'zzzz' },
  { name: 'no-action', typed: '>release' },
  { name: 'chats-loading', settings: ['Loading'], typed: 'release' },
  { name: 'chats-failed', settings: ['Failed'], typed: 'release' },
  { name: 'no-chats-yet', settings: ['None'] },
  { name: 'key-table-mac', part: 'key-table' },
  { name: 'key-table-windows-linux', settings: ['Windows and Linux'], part: 'key-table' },
  { name: 'clashes', part: 'clashes' },
];

async function openPalette(page: Page, shot: PaletteShot): Promise<void> {
  await page.getByRole('button', { name: 'Open the command palette' }).click();
  const field = page.getByRole('combobox', { name: 'Search commands and chats' });
  await expect(field).toBeFocused();
  if (shot.typed !== undefined) await field.fill(shot.typed);
  for (const key of shot.keys ?? []) await page.keyboard.press(key);
  await page.mouse.move(0, 0);
}

for (const chosen of MODES.filter((mode) => !mode.dyslexic && mode.width === 1280)) {
  test.describe(`code-palette-${chosen.folder}`, () => {
    test.skip(process.env['GALLERY_CODE_PALETTE'] !== '1', 'The pictures of the command palette are made only when asked.');
    test.use({ viewport: { width: chosen.width, height: HEIGHT }, colorScheme: chosen.theme });

    test.beforeEach(async ({ page, context }) => {
      mkdirSync(`${FOLDER}${chosen.folder}`, { recursive: true });
      await context.addInitScript((settings) => {
        for (const [key, value] of Object.entries(settings)) localStorage.setItem(key, value);
      }, storedSettings(chosen));
      const hub = await installSyntheticHub(context, {});
      await hub.signIn();
      await page.goto(`${HUB_ORIGIN}/code-kit/palette`);
      await expect(page.getByRole('heading', { level: 1, name: 'Command palette' })).toBeVisible();
      await page.evaluate(() => document.fonts.ready);
    });

    test('page', async ({ page }) => {
      await page.waitForTimeout(SETTLE_MS);
      await page.screenshot({ path: `${FOLDER}${chosen.folder}/00-page.png`, fullPage: true });
    });

    SHOTS.forEach((shot, index) => {
      test(shot.name, async ({ page }) => {
        for (const name of shot.settings ?? []) await page.getByRole('radio', { name, exact: true }).click();
        const file = `${FOLDER}${chosen.folder}/${String(index + 1).padStart(2, '0')}-${shot.name}.png`;
        if (shot.part !== undefined) {
          const part = page.locator(`[data-kit="${shot.part}"]`);
          await page.waitForTimeout(SETTLE_MS);
          await part.screenshot({ path: file });
          return;
        }
        await openPalette(page, shot);
        await page.waitForTimeout(SETTLE_MS);
        await page.screenshot({ path: file });
      });
    });
  });
}
