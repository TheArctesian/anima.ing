import { expect, type Page } from '@playwright/test';
import { HUB_ORIGIN } from '../synthetic/access';

export const DEVICE_IDS = {
  winLegacy: '0190a000-0000-7000-8012-000000000002',
  macMini: '0190a000-0000-7000-8012-000000000003',
  gpuBox: '0190a000-0000-7000-8012-000000000004',
  nas: '0190a000-0000-7000-8012-000000000005',
} as const;

export async function openDevices(page: Page): Promise<void> {
  await page.goto(`${HUB_ORIGIN}/devices`, { waitUntil: 'domcontentloaded' });
  await expect(page.getByRole('link', { name: 'mac-studio', exact: true })).toBeVisible();
  await expect(page.getByRole('region', { name: 'Logs' }).getByText('Sign-in refused', { exact: true })).toBeVisible();
}
