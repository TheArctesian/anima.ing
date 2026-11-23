import { expect, type Locator, type Page } from '@playwright/test';
import { HEIGHT } from '../shots';

export const card = (inbox: Locator, title: string): Locator => inbox.getByRole('button', { name: new RegExp(title) }).first();

export async function newThread(page: Page, inbox: Locator, picking: boolean): Promise<readonly Locator[]> {
  await card(inbox, 'Review the sign-in flow').click();
  await inbox.getByRole('button', { name: 'New thread' }).click();
  await page.mouse.move(1000, HEIGHT / 2);
  const heading = page.getByRole('heading', { name: /What should we build in/ });
  await expect(heading).toBeVisible();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  if (picking) {
    await heading.getByRole('button').click();
    await expect(page.getByRole('listbox', { name: 'Projects' })).toBeVisible();
  }
  return [];
}

export async function openModels(page: Page, inbox: Locator, program: string | null, listed = true): Promise<readonly Locator[]> {
  await newThread(page, inbox, false);
  await page.getByRole('button', { name: /^Claude Code default/ }).click();
  const picker = page.getByRole('dialog', { name: 'Model' });
  await expect(listed ? picker.getByText('Opus 4.7') : picker.getByRole('button', { name: 'Retry' })).toBeVisible();
  if (program !== null) await picker.getByRole('button', { name: program, exact: true }).click();
  await page.mouse.move(1000, HEIGHT - 20);
  return [picker];
}


export async function addProject(page: Page, inbox: Locator, browse: boolean): Promise<readonly Locator[]> {
  await newThread(page, inbox, true);
  await page.getByRole('option', { name: 'Add project' }).click();
  const palette = page.getByRole('dialog', { name: 'Add project' });
  await expect(palette.getByText('Sources')).toBeVisible();
  if (browse) {
    await page.keyboard.press('Enter');
    await expect(palette.getByRole('option', { name: 'spikes' })).toBeVisible();
  }
  await page.mouse.move(1000, HEIGHT - 20);
  return [palette];
}
