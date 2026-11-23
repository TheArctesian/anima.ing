import { mkdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test } from '@playwright/test';
import { HUB_ORIGIN } from '../../synthetic/access';
import { installSyntheticHub } from '../../synthetic/synthetic-hub';
import { HEIGHT, MODES, storedSettings } from '../shots';
import { installDesktopShell, type Scenario } from '../threads/desktop-shell';
import { EVERY_STATE } from '../threads/scenario';

// A Codex thread not started in Anima, from the page test's capture of a real session file (code mode: each tool one
// `exec` of JavaScript), opened on the page Code: T3 Code's timeline, the work group and its commands open.
// `GALLERY_CODE_SHELL=1`; pictures go to `gallery-shots/code-shell/<theme>-1440/21-stored-codex.png`.

const FOLDER = fileURLToPath(new URL('../../../gallery-shots/code-shell/', import.meta.url));
const CAPTURE = JSON.parse(readFileSync(new URL('../../../src/lib/code/chat/tests/codex-code-mode-thread.json', import.meta.url), 'utf8')) as Record<string, unknown>;
const STORED: Scenario = {
  ...EVERY_STATE,
  rows: [{ ...CAPTURE, entries: undefined, started: '@-30', last: '@-28' }, ...EVERY_STATE.rows],
  threads: { ...EVERY_STATE.threads, [String(CAPTURE['id'])]: CAPTURE },
};

for (const chosen of MODES.filter((mode) => !mode.dyslexic && mode.width === 1280)) {
  const wide = { ...chosen, width: 1440, folder: `${chosen.theme}-1440` };
  test.describe(`stored-thread-${wide.folder}`, () => {
    test.skip(process.env['GALLERY_CODE_SHELL'] !== '1', 'The pictures of the Code page are made only when asked.');
    test.use({ viewport: { width: 1440, height: HEIGHT }, colorScheme: chosen.theme });
    test('21-stored-codex.png', async ({ page }) => {
      mkdirSync(`${FOLDER}${wide.folder}`, { recursive: true });
      await page.context().addInitScript((settings) => {
        for (const [key, value] of Object.entries(settings)) localStorage.setItem(key, value);
      }, storedSettings(wide));
      await installDesktopShell(page.context(), STORED);
      const hub = await installSyntheticHub(page.context(), {});
      await hub.signIn();
      await page.goto(`${HUB_ORIGIN}/code`);
      const inbox = page.getByRole('complementary', { name: 'Threads of this computer' });
      await inbox.getByText(/^Codex in e2e-1430, /).first().click();
      await page.getByRole('button', { name: /^Worked for/ }).click();
      await page.getByRole('button', { name: /^Ran 2 commands/ }).click();
      await expect(page.getByText('pwd -P and 1 more')).toBeVisible();
      await page.evaluate(() => document.fonts.ready);
      await page.waitForTimeout(400);
      await page.screenshot({ path: `${FOLDER}${wide.folder}/21-stored-codex.png` });
    });
  });
}
