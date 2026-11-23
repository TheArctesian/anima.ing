import { expect, type Locator, type Page } from '@playwright/test';
import type { Scenario } from '../threads/desktop-shell';
import { IDS } from '../threads/scenario';

/** The threads the runner started; the other three sessions of the scenario began outside Anima (operator, 2026-11-23). */
const OURS: readonly string[] = [IDS.asks, IDS.working, IDS.starting, IDS.talked, IDS.yourTurn, IDS.writing, IDS.pinnedFile, IDS.archivedA, IDS.archivedB].map(String);

/** `base` with a runner that marks the threads it started. */
export function outsideMarked(base: Scenario): Scenario {
  const marked = base.listings.map((listing) => ({ ...listing, started_by_anima: OURS.includes(String(listing['thread'])) }));
  const unlisted = OURS.filter((thread) => !base.listings.some((listing) => listing['thread'] === thread));
  return { ...base, listings: [...marked, ...unlisted.map((thread) => ({ thread, pinned: false, archived: false, title: null, removed: false, started_by_anima: true }))] };
}

/** The filter with its switch for the sessions that Anima did not start; `show` turns it on. */
export async function outsideFilter(page: Page, inbox: Locator, show: boolean): Promise<readonly Locator[]> {
  await inbox.getByRole('button', { name: /Filter threads by project/ }).click();
  const panel = page.getByRole('listbox', { name: 'Projects' }).locator('..');
  const toggle = panel.getByRole('switch', { name: 'Show sessions started outside Anima' });
  await expect(toggle).toHaveAttribute('aria-checked', 'false');
  if (show) {
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'true');
  }
  return [panel];
}
