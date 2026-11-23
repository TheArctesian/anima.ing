import { expect, type BrowserContext, type Page } from '@playwright/test';
import { HUB_ORIGIN, OPERATOR, SESSION_COOKIE } from './synthetic/access';

// The one definition of "no token or password in reach of script" (CLI-003, D-3).

export function consoleLines(page: Page): string[] {
  const lines: string[] = [];
  page.on('console', (message) => lines.push(message.text()));
  return lines;
}

export function scriptReach(page: Page): Promise<string> {
  return page.evaluate(() => {
    const values = [...document.querySelectorAll('input')].map((field) => field.value);
    return JSON.stringify([document.cookie, { ...localStorage }, { ...sessionStorage }, document.documentElement.outerHTML, values]);
  });
}

export async function sessionCookie(context: BrowserContext): Promise<string | undefined> {
  return (await context.cookies(HUB_ORIGIN)).find((found) => found.name === SESSION_COOKIE)?.value;
}

export async function continueWith(page: Page, password: string): Promise<void> {
  await page.getByLabel('User name', { exact: true }).fill(OPERATOR.username);
  await page.getByLabel('Password', { exact: true }).fill(password);
  await page.getByRole('button', { name: 'Continue', exact: true }).click();
}

export async function signInWith(page: Page, password: string, code: string = OPERATOR.code): Promise<void> {
  await continueWith(page, password);
  if (code === '') {
    await page.getByRole('button', { name: 'Sign in without a code', exact: true }).click();
    return;
  }
  // Each digit moves focus to the next box, and the sixth sends the code.
  await expect(page.getByLabel('Digit 1 of 6')).toBeFocused();
  await page.keyboard.type(code);
}

/** The specs need the form, not the `load` event (fonts, icons). */
export async function openSignIn(page: Page): Promise<void> {
  await page.goto(`${HUB_ORIGIN}/sign-in`, { waitUntil: 'domcontentloaded' });
  await expect(page.getByRole('button', { name: 'Continue', exact: true })).toBeEnabled();
}
