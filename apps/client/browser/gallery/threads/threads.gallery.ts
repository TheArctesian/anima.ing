import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { HUB_ORIGIN } from '../../synthetic/access';
import { installSyntheticHub } from '../../synthetic/synthetic-hub';
import { HEIGHT, MODES, storedSettings } from '../shots';
import { installDesktopShell, type Scenario } from './desktop-shell';
import { EVERY_STATE, HOOKED_FOLDER, OLDER_HOLDER, RUNNER_DOWN, STARTS } from './scenario';

// Only the desktop application shows the page Code, so the gallery stands in for its shell (THR-011 to THR-017).

const FOLDER = fileURLToPath(new URL('../../../gallery-shots/threads/', import.meta.url));
const SETTLE_MS = 400;
/** At this width and below the thread stands under the list, so the picture is the whole page (60rem). */
const NARROW = 960;

interface ThreadsScreen {
  readonly name: string;
  readonly scenario: Scenario;
  readonly open: (page: Page) => Promise<void>;
  readonly windowOnly?: true;
}

async function openThreads(page: Page): Promise<void> {
  await page.goto(`${HUB_ORIGIN}/code`);
  await expect(page.getByRole('heading', { level: 1, name: 'Code' })).toBeVisible();
  await expect(page.getByRole('searchbox', { name: 'Search threads' }).or(page.getByLabel('Search threads'))).toBeVisible();
}

async function openThread(page: Page, title: string): Promise<void> {
  await openThreads(page);
  await page.getByRole('button', { name: new RegExp(title) }).first().click();
  await expect(page.getByRole('heading', { level: 2, name: title })).toBeVisible();
}

async function startDialog(page: Page, folder: string): Promise<void> {
  await openThreads(page);
  await page.getByRole('button', { name: 'New thread' }).click();
  const dialog = page.getByRole('dialog', { name: 'New thread' });
  await expect(dialog).toBeVisible();
  // The field Folder has no name of its own for a reader of the screen: it is found by its example.
  await dialog.getByPlaceholder('/Users/you/project').fill(folder);
  await dialog.getByLabel('First prompt').fill('Make the needs-you count of the bar match the list.');
  await dialog.getByRole('button', { name: 'Start' }).click();
}

const SCREENS: readonly ThreadsScreen[] = [
  {
    name: 'list',
    scenario: EVERY_STATE,
    open: async (page) => {
      await openThreads(page);
      await expect(page.getByText('Asks you').first()).toBeVisible();
    },
  },
  {
    name: 'list-earlier-open',
    scenario: EVERY_STATE,
    open: async (page) => {
      await openThreads(page);
      await page.getByRole('button', { name: /Earlier/ }).click();
      await expect(page.getByText('Tidy the device page')).toBeVisible();
    },
  },
  {
    name: 'archived',
    scenario: EVERY_STATE,
    open: async (page) => {
      await openThreads(page);
      await page.getByRole('button', { name: 'Archived (2)' }).click();
      // Archived threads are done, so they come in the group Earlier; it opens with the view, or is opened here.
      const earlier = page.getByRole('button', { name: /Earlier/ });
      if ((await earlier.getAttribute('aria-expanded')) === 'false') await earlier.click();
      await expect(page.getByText('Spike: WebGPU charts')).toBeVisible();
    },
  },
  {
    name: 'thread-open',
    scenario: EVERY_STATE,
    windowOnly: true,
    open: async (page) => {
      await openThread(page, 'Why does the gates run stop at limits?');
      await expect(page.getByText('So the way out of a failed limit')).toBeVisible();
    },
  },
  {
    name: 'approval-diff',
    scenario: EVERY_STATE,
    windowOnly: true,
    open: async (page) => {
      await openThread(page, 'Group the accounts on Agent usage');
      await expect(page.getByRole('button', { name: 'Allow once' })).toBeVisible();
    },
  },
  {
    name: 'working-remove-refused',
    scenario: EVERY_STATE,
    windowOnly: true,
    open: async (page) => {
      await openThread(page, 'Fix the flaky batching test');
      await expect(page.getByText(/first to remove it\./)).toBeVisible();
    },
  },
  {
    name: 'starting-waits',
    scenario: EVERY_STATE,
    windowOnly: true,
    open: async (page) => {
      await openThread(page, 'Explain the merge train');
      await expect(page.getByText(/Waiting for the first answer of the program/)).toBeVisible();
    },
  },
  {
    name: 'new-thread-folder-notice',
    scenario: STARTS,
    windowOnly: true,
    open: async (page) => {
      await startDialog(page, HOOKED_FOLDER);
      await expect(page.getByRole('note', { name: 'What this folder runs by itself' })).toBeVisible();
    },
  },
  {
    name: 'new-thread-other-copy',
    scenario: STARTS,
    windowOnly: true,
    open: async (page) => {
      await startDialog(page, '/Users/you/anima');
      await expect(page.getByRole('button', { name: 'Start with this copy' })).toBeVisible();
    },
  },
  {
    name: 'rename',
    scenario: EVERY_STATE,
    windowOnly: true,
    open: async (page) => {
      await openThread(page, 'Why does the gates run stop at limits?');
      await page.getByRole('button', { name: 'Rename' }).click();
      await expect(page.getByRole('dialog', { name: 'Rename the thread' })).toBeVisible();
    },
  },
  {
    name: 'remove-question',
    scenario: EVERY_STATE,
    windowOnly: true,
    open: async (page) => {
      // A thread whose program does not run: the runner removes only such a thread.
      await openThread(page, 'Port the battery meter');
      await page.getByRole('button', { name: 'Remove' }).click();
      await expect(page.getByRole('alertdialog', { name: 'Remove from the list' })).toBeVisible();
    },
  },
  {
    name: 'holder-older',
    scenario: OLDER_HOLDER,
    open: async (page) => {
      await openThreads(page);
      await expect(page.getByText(/older than the daemon/)).toBeVisible();
    },
  },
  {
    name: 'runner-not-running',
    scenario: RUNNER_DOWN,
    open: async (page) => {
      await openThreads(page);
      await expect(page.getByText(/thread runner of this computer is not running/)).toBeVisible();
    },
  },
];

const numbered = SCREENS.map((screen, index) => ({ screen, file: `${String(index + 1).padStart(2, '0')}-${screen.name}.png` }));

for (const chosen of MODES.filter((mode) => !mode.dyslexic)) {
  test.describe(`threads-${chosen.folder}`, () => {
    test.skip(process.env['GALLERY_THREADS'] !== '1', 'The pictures of the page Code are made only when asked.');
    test.use({ viewport: { width: chosen.width, height: HEIGHT }, colorScheme: chosen.theme });
    for (const { screen, file } of numbered) {
      test(file, async ({ page, context }) => {
        mkdirSync(`${FOLDER}${chosen.folder}`, { recursive: true });
        await context.addInitScript((settings) => {
          for (const [key, value] of Object.entries(settings)) localStorage.setItem(key, value);
        }, storedSettings(chosen));
        await installDesktopShell(context, screen.scenario);
        const hub = await installSyntheticHub(context, {});
        await hub.signIn();
        await screen.open(page);
        await page.evaluate(() => document.fonts.ready);
        await page.waitForTimeout(SETTLE_MS);
        await page.screenshot({ path: `${FOLDER}${chosen.folder}/${file}`, fullPage: screen.windowOnly !== true || chosen.width <= NARROW });
      });
    }
  });
}
