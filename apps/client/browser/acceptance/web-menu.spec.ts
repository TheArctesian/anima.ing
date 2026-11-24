import { expect, test } from '../test-hub';
import { openDevices } from './screens';

// Chromium fires a `scroll` about 130 ms after a plain click, and a mobile address bar fires `resize` with the same width (WEB-011, MOT-002).

const MENU = { name: 'Web interfaces of nas' };
const STAYS_OPEN_MS = 500;

test.use({ signedIn: true });

test('WEB-011: the menu stays open for 500 ms after a plain click', async ({ page }) => {
  await openDevices(page);
  await page.getByRole('button', MENU).click();
  const menu = page.getByRole('group', MENU);
  await expect(menu).toBeVisible();
  await page.waitForTimeout(STAYS_OPEN_MS);
  await expect(menu).toBeVisible();
});

test('MOT-002: a real wheel scroll that moves the button closes the menu', async ({ page }) => {
  await openDevices(page);
  // The page must be able to scroll by more than the tolerance, whatever the fixtures hold.
  await page.addStyleTag({ content: 'body::after { content: ""; display: block; block-size: 3000px; }' });
  const button = page.getByRole('button', MENU);
  await button.scrollIntoViewIfNeeded();
  await button.click();
  const menu = page.getByRole('group', MENU);
  await expect(menu).toBeVisible();
  const before = await button.boundingBox();
  await page.mouse.move(5, 5);
  await page.mouse.wheel(0, 200);
  await expect.poll(async () => (await button.boundingBox())?.y).not.toBe(before?.y);
  await expect(menu).toHaveCount(0);
});

test('WEB-011: a resize that keeps the width keeps the menu open, and a width change closes it', async ({ page }) => {
  await openDevices(page);
  const { width, height } = page.viewportSize() ?? { width: 1280, height: 720 };
  await page.getByRole('button', MENU).click();
  const menu = page.getByRole('group', MENU);
  await expect(menu).toBeVisible();

  await page.setViewportSize({ width, height: height - 80 });
  await page.waitForTimeout(STAYS_OPEN_MS);
  await expect(menu).toBeVisible();

  await page.setViewportSize({ width: width - 120, height: height - 80 });
  await expect(menu).toHaveCount(0);
});

test('WEB-011: a height-only resize that cuts the panel keeps the open menu inside the window and next to its button', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 700 });
  await openDevices(page);
  const button = page.getByRole('button', MENU);
  await button.click();
  const menu = page.getByRole('group', MENU);
  await expect(menu).toBeVisible();

  const opened = await menu.boundingBox();
  if (opened === null) throw new Error('menu has no box');
  const CUT_PX = 40;
  const shorter = Math.floor(opened.y + opened.height) - CUT_PX;
  await page.setViewportSize({ width: 390, height: shorter });
  await page.waitForTimeout(STAYS_OPEN_MS);
  await expect(menu).toBeVisible();

  const inside = await page.evaluate(() => {
    const height = document.documentElement.clientHeight;
    const parts = [...document.querySelectorAll('[role=group][aria-label^="Web interfaces"] .choice, [role=group][aria-label^="Web interfaces"] .pinned')];
    return parts.map((part) => part.getBoundingClientRect()).map((rect) => rect.top >= 0 && rect.bottom <= height);
  });
  expect(inside.length).toBeGreaterThanOrEqual(3); // two choices and the pin of the pinned one
  expect(inside.every(Boolean)).toBe(true);

  const panel = await menu.boundingBox();
  const anchor = await button.boundingBox();
  if (panel === null || anchor === null) throw new Error('menu or button has no box');
  const GAP_PX = 16;
  const under = panel.y - (anchor.y + anchor.height);
  const above = anchor.y - (panel.y + panel.height);
  expect(Math.min(Math.abs(under), Math.abs(above))).toBeLessThanOrEqual(GAP_PX);
});
