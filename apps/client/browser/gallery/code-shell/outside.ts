import { expect, type Locator, type Page } from '@playwright/test';
import type { Scenario } from '../threads/desktop-shell';
import { IDS } from '../threads/scenario';

/** The threads the runner started; the other three sessions of the scenario began outside Anima (operator, 2026-11-23). */
const OURS: readonly string[] = [IDS.asks, IDS.working, IDS.starting, IDS.talked, IDS.yourTurn, IDS.writing, IDS.pinnedFile, IDS.archivedA, IDS.archivedB].map(String);

export function outsideMarked(base: Scenario, ours = OURS): Scenario {
  const marked = base.listings.map((listing) => ({ ...listing, started_by_anima: ours.includes(String(listing['thread'])) }));
  const unlisted = ours.filter((thread) => !base.listings.some((listing) => listing['thread'] === thread));
  return { ...base, listings: [...marked, ...unlisted.map((thread) => ({ thread, pinned: false, archived: false, title: null, removed: false, started_by_anima: true }))] };
}

export async function outsideFilter(page: Page, inbox: Locator, show: boolean): Promise<readonly Locator[]> {
  const started = inbox.getByText('Review the sign-in flow', { exact: true });
  const imported = inbox.getByText('Bump the Rust toolchain to 1.92', { exact: true });
  await expect(started).toBeVisible();
  await expect(imported).toHaveCount(0);
  await inbox.getByRole('button', { name: /Filter threads by project/ }).click();
  const panel = page.getByRole('listbox', { name: 'Projects' }).locator('..');
  const toggle = panel.getByRole('switch', { name: 'Browse imported history' });
  await expect(toggle).toHaveAttribute('aria-checked', 'false');
  if (show) {
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'true');
    await expect(inbox.getByText('Imported history', { exact: true })).toBeVisible();
    await expect(started).toHaveCount(0);
    await expect(imported).toBeVisible();
  }
  return [panel];
}
