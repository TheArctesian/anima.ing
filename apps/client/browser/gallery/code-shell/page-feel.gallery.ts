import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { HUB_ORIGIN } from '../../synthetic/access';
import { installSyntheticHub } from '../../synthetic/synthetic-hub';
import { HEIGHT, MODES, storedSettings } from '../shots';
import { installDesktopShell, type Scenario } from '../threads/desktop-shell';
import { OLDER_HOLDER } from '../threads/scenario';
import { card, newThread } from './draft';

// How the page feels on a busy computer: the runner's line in one row with its stop, the chat's model by the runner's
// name, and a new thread's prompt at once. `GALLERY_CODE_SHELL=1`; pictures go to `gallery-shots/code-shell/<mode>/`.

const FOLDER = fileURLToPath(new URL('../../../gallery-shots/code-shell/', import.meta.url));
const MODELS = [{ program: 'claude_code', models: [{ value: 'opus', name: 'Opus 5.5', efforts: ['low', 'medium', 'high', 'xhigh', 'max'], default_effort: null }] }];
const HOLDER: Scenario = { ...OLDER_HOLDER, models: MODELS };
/** A thread the send starts: the runner lists it, and the program has not taken its prompt yet. */
const FRESH = { id: 'th-fresh', program: 'claude_code', folder: '', mode: 'full_access', model: 'claude-opus-5-5', session: null, ended: null, open: [], title: 'New thread', state: 'starting', started: '@-0.1' };
const SENDING: Scenario = { ...OLDER_HOLDER, holder: undefined, models: MODELS, running: [FRESH, ...OLDER_HOLDER.running] } as Scenario;

async function opened(page: Page, scenario: Scenario, settings: Record<string, string>): Promise<void> {
  await page.context().addInitScript((stored) => {
    for (const [key, value] of Object.entries(stored)) localStorage.setItem(key, value);
  }, settings);
  await installDesktopShell(page.context(), scenario);
  const hub = await installSyntheticHub(page.context(), {});
  await hub.signIn();
  await page.goto(`${HUB_ORIGIN}/code`);
}

async function shot(page: Page, path: string): Promise<void> {
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(400);
  await page.screenshot({ path });
}

for (const chosen of MODES.filter((mode) => !mode.dyslexic && mode.width === 1280)) {
  test.describe(`page-feel-${chosen.folder}`, () => {
    test.skip(process.env['GALLERY_CODE_SHELL'] !== '1', 'The pictures of the Code page are made only when asked.');
    test.use({ viewport: { width: chosen.width, height: HEIGHT }, colorScheme: chosen.theme });
    test.beforeAll(() => mkdirSync(`${FOLDER}${chosen.folder}`, { recursive: true }));

    test('25-holder-line.png', async ({ page }) => {
      await opened(page, HOLDER, storedSettings(chosen));
      const inbox = page.getByRole('complementary', { name: 'Threads of this computer' });
      await card(inbox, 'Why does the gates run stop at limits').click();
      await expect(page.getByRole('button', { name: 'Stop the 2 running threads and replace' })).toBeVisible();
      await page.mouse.move(chosen.width / 2, HEIGHT / 2);
      await shot(page, `${FOLDER}${chosen.folder}/25-holder-line.png`);
      await page.getByRole('button', { name: 'Stop the 2 running threads and replace' }).click();
      await expect(page.getByRole('button', { name: 'Stop and replace' })).toBeVisible();
      await shot(page, `${FOLDER}${chosen.folder}/26-holder-confirm.png`);
    });

    test('27-sent-prompt.png', async ({ page }) => {
      await opened(page, SENDING, storedSettings(chosen));
      const inbox = page.getByRole('complementary', { name: 'Threads of this computer' });
      await newThread(page, inbox, false);
      await page.getByRole('button', { name: 'or start without a project' }).click();
      await page.getByRole('textbox', { name: /Prompt|Message/ }).fill('Explain why the gate stops at the limits');
      await page.keyboard.press('Enter');
      await expect(page.getByText('Explain why the gate stops at the limits')).toBeVisible();
      await page.mouse.move(chosen.width / 2, HEIGHT / 2);
      await shot(page, `${FOLDER}${chosen.folder}/27-sent-prompt.png`);
    });
  });
}
