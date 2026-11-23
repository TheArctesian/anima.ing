import { expect, type Page, type Route } from '@playwright/test';
import { HUB_ORIGIN } from '../synthetic/access';
import { DEVICE_IDS as FIXTURE_DEVICES, openDevices } from '../acceptance/screens';

export { openDevices };
export const DEVICE_IDS = { ...FIXTURE_DEVICES, winDesktop: '0190a000-0000-7000-8012-000000000001' } as const;

export async function openDevice(page: Page, id: string, name: string): Promise<void> {
  await page.goto(`${HUB_ORIGIN}/devices/${id}`);
  await expect(page.getByRole('heading', { level: 1, name })).toBeVisible();
}

export async function openGpuBox(page: Page, tab?: string): Promise<void> {
  await openDevice(page, DEVICE_IDS.gpuBox, 'gpu-box');
  if (tab !== undefined) await page.getByRole('tab', { name: tab }).click();
}

export async function openSettings(page: Page): Promise<void> {
  await page.goto(`${HUB_ORIGIN}/settings`);
  await expect(page.getByRole('heading', { level: 2, name: 'API keys' })).toBeVisible();
  await expect(page.getByText('router-lab').first()).toBeVisible();
}

export const section = (page: Page, name: string) => page.getByRole('region', { name, exact: true });

export async function openedDialog(page: Page, name?: string) {
  const dialog = name === undefined ? page.getByRole('dialog') : page.getByRole('dialog', { name });
  await expect(dialog.first()).toBeVisible();
  return dialog.first();
}

export async function onOperations(page: Page, operations: readonly string[], answer: (route: Route) => Promise<void>): Promise<void> {
  await page.route('**/graphql', async (route) => {
    const body = JSON.parse(route.request().postData() ?? '{}') as { query?: string; operationName?: string };
    const name = body.operationName ?? /^\s*(?:query|mutation)\s+(\w+)/.exec(body.query ?? '')?.[1] ?? '';
    if (route.request().method() === 'POST' && operations.includes(name)) await answer(route);
    else await route.fallback();
  });
}

export function failOperations(page: Page, operations: readonly string[]): Promise<void> {
  const body = JSON.stringify({ data: null, errors: [{ message: 'The hub could not read its database.', extensions: { code: 'INTERNAL' } }] });
  return onOperations(page, operations, (route) => route.fulfill({ status: 200, contentType: 'application/graphql-response+json', body }));
}

export function holdOperations(page: Page, operations: readonly string[]): Promise<void> {
  return onOperations(page, operations, () => new Promise<void>(() => undefined));
}
