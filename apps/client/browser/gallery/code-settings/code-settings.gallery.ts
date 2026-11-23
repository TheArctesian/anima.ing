import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { HUB_ORIGIN } from '../../synthetic/access';
import { installSyntheticHub } from '../../synthetic/synthetic-hub';
import { HEIGHT, MODES, storedSettings, type Mode } from '../shots';
import { section } from '../steps';
import { installDesktopShell } from '../threads/desktop-shell';
import { EVERY_STATE } from '../threads/scenario';

const FOLDER = fileURLToPath(new URL('../../../gallery-shots/code-settings/', import.meta.url));
const SETTLE_MS = 400;
const WIDTH = 1440;

/** The reader settings as `lib/code/settings/reader.ts` saves them. */
type Reader = Readonly<Record<'anima.code.width' | 'anima.code.contrast' | 'anima.code.font-size', string>>;

async function prepare(page: Page, mode: Mode, reader: Partial<Reader> = {}): Promise<void> {
  await page.context().addInitScript((settings) => {
    for (const [key, value] of Object.entries(settings)) localStorage.setItem(key, value);
  }, { ...storedSettings(mode), ...reader });
  await installDesktopShell(page.context(), EVERY_STATE);
  const hub = await installSyntheticHub(page.context(), {});
  await hub.signIn();
}

async function openSettingsAtCode(page: Page): Promise<void> {
  await page.goto(`${HUB_ORIGIN}/settings`);
  await expect(section(page, 'Code appearance')).toBeVisible();
}

async function openCodeThread(page: Page): Promise<void> {
  await page.goto(`${HUB_ORIGIN}/code`);
  const inbox = page.getByRole('complementary', { name: 'Threads of this computer' });
  await inbox.getByRole('button', { name: /Why does the gates run stop at limits/ }).first().click();
  await expect(page.getByText('So the way out of a failed limit')).toBeVisible();
}

async function shoot(page: Page, path: string, target?: () => ReturnType<typeof section>): Promise<void> {
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(SETTLE_MS);
  if (target === undefined) await page.screenshot({ path });
  else await target().screenshot({ path });
}

const PAGES: readonly (readonly [string, Partial<Reader>])[] = [
  ['04-code-default.png', {}],
  ['05-code-wide-18px-contrast-200.png', { 'anima.code.width': 'wide', 'anima.code.font-size': '18', 'anima.code.contrast': '200' }],
  ['06-code-full-20px.png', { 'anima.code.width': 'full', 'anima.code.font-size': '20' }],
];

const run = process.env['GALLERY_CODE_SETTINGS'] === '1';

for (const theme of ['dark', 'light'] as const) {
  const base = MODES.find((each) => each.theme === theme && !each.dyslexic);
  if (base === undefined) continue;
  const mode: Mode = { ...base, width: WIDTH, folder: `${theme}-${String(WIDTH)}` };
  const folder = `${FOLDER}${mode.folder}`;

  test.describe(`code-settings-${mode.folder}`, () => {
    test.skip(!run, 'The pictures of the Code settings are made only when asked.');
    test.use({ viewport: { width: WIDTH, height: HEIGHT }, colorScheme: theme });
    test.beforeEach(() => {
      mkdirSync(folder, { recursive: true });
    });

    test('01-section-default.png', async ({ page }) => {
      await prepare(page, mode);
      await openSettingsAtCode(page);
      await shoot(page, `${folder}/01-section-default.png`, () => section(page, 'Code appearance'));
    });

    test('02-section-changed.png', async ({ page }) => {
      await prepare(page, mode);
      await openSettingsAtCode(page);
      await page.locator('#code-contrast').fill('150');
      await page.locator('#code-chat-width').selectOption('wide');
      await page.locator('#code-font-size').selectOption('16');
      await expect(page.getByRole('button', { name: 'Reset font size to default' })).toBeVisible();
      await shoot(page, `${folder}/02-section-changed.png`, () => section(page, 'Code appearance'));
    });

    test('03-settings-index.png', async ({ page }) => {
      await prepare(page, mode);
      await openSettingsAtCode(page);
      await section(page, 'Code appearance').scrollIntoViewIfNeeded();
      await shoot(page, `${folder}/03-settings-index.png`);
    });

    for (const [name, reader] of PAGES) {
      test(name, async ({ page }) => {
        await prepare(page, mode, reader);
        await openCodeThread(page);
        await shoot(page, `${folder}/${name}`);
      });
    }
  });
}
