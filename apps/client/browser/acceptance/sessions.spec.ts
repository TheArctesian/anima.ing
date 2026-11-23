import { expect, test } from '../test-hub';
import { k7Policy } from '../csp/k7-policy';
import { expectNoViolations, servePolicy, violationsSoFar, type PolicyReports } from '../csp/violations';
import { HUB_ORIGIN } from '../synthetic/access';
import { TERMINAL_TITLE } from '../synthetic/tunnels';
import { openDevices } from './screens';

// Plan criterion 2, client side: each session runs under the exact K7 policy with a report channel (WP-44 T5, SEC-001).

let policy: PolicyReports;

test.use({ signedIn: true });
test.beforeEach(async ({ context }) => {
  policy = await servePolicy(context, k7Policy());
});
test.afterEach(({ watch }) => {
  expect(violationsSoFar(watch, policy)).toEqual({ events: [], reports: [] });
});

test('RA-007, D-5: an SSH session opens over the terminal tunnel and shows what the shell sent, under K7 with no violation', async ({
  page,
  hub,
  watch,
}) => {
  await openDevices(page);
  await page.getByRole('button', { name: 'SSH to gpu-box' }).click();

  // The tab title is the shell's own window title, set by bytes that came over the tunnel and went through xterm.js.
  const tab = page.getByRole('tab', { name: /gpu-box/ });
  await expect(tab).toContainText(TERMINAL_TITLE);
  await expect(page.getByRole('group', { name: 'Terminal of gpu-box' }).locator('.xterm')).toBeVisible();
  expect(hub.recording.where({ channel: 'terminal' }).map((found) => found.operation)).toEqual(['ticket accepted']);
  expect(hub.recording.where({ operation: 'StartSession' })).toHaveLength(1);
  expect(watch.pageErrors).toEqual([]);
  await expectNoViolations(page, watch, policy);
});

test('RA-001, D-5: a VNC session opens over the Guacamole tunnel and draws the remote screen, under K7 with no violation', async ({
  page,
  hub,
  watch,
}) => {
  await openDevices(page);
  await page.getByRole('button', { name: 'VNC to mac-mini' }).click();
  await expect(page.getByRole('tab', { name: /mac-mini/ })).toBeVisible();
  await expect.poll(() => hub.recording.where({ channel: 'tunnel', operation: 'ticket accepted' }).length).toBe(1);

  // The synthetic hub fills the screen with one colour (`cfill`); a canvas holding it shows the drawing reached the page.
  const filled = page.locator('canvas');
  await expect
    .poll(() =>
      filled.evaluateAll((canvases) =>
        canvases.some((canvas) => {
          const pixel = (canvas as HTMLCanvasElement).getContext('2d')?.getImageData(5, 5, 1, 1).data;
          return pixel !== undefined && pixel[0] === 32 && pixel[1] === 64 && pixel[2] === 96;
        }),
      ),
    )
    .toBe(true);
  expect(watch.pageErrors).toEqual([]);
  await expectNoViolations(page, watch, policy);
});

test('M4-3: VNC uses the iPad viewport with a narrow margin and Back keeps the session', async ({ page, hub }) => {
  await page.setViewportSize({ width: 1366, height: 1024 });
  await page.addInitScript(() => Object.defineProperty(navigator, 'userAgent', { get: () => 'Mozilla/5.0 (iPad; CPU OS 26_7 like Mac OS X) AppleWebKit/605.1.15' }));
  await page.goto(`${HUB_ORIGIN}/devices`);
  await page.getByRole('button', { name: 'VNC to mac-mini' }).click();
  const session = page.getByRole('region', { name: 'VNC session on mac-mini' });
  await expect(session).toBeVisible();
  await expect(page.locator('#main-nav')).toBeHidden();
  expect(await session.boundingBox()).toMatchObject({ x: 8, y: 8, width: 1350, height: 1008 });
  await expect(page.locator('[data-display] canvas').first()).toBeVisible();

  await page.setViewportSize({ width: 1024, height: 1366 });
  await expect.poll(() => session.boundingBox()).toMatchObject({ x: 8, y: 8, width: 1008, height: 1350 });

  await page.getByRole('button', { name: 'Back to app' }).click();
  await expect(page.locator('#main-nav')).toBeVisible();
  await page.getByRole('tab', { name: /mac-mini/ }).click();
  await expect(session).toBeVisible();
  expect(hub.recording.where({ operation: 'StartSession' })).toHaveLength(1);
});

test('RA-002, CLI-001: a session tab offers no Pop out', async ({ page }) => {
  await openDevices(page);
  await page.getByRole('button', { name: 'SSH to gpu-box' }).click();
  await expect(page.getByRole('tab', { name: /gpu-box/ })).toBeVisible();
  await expect(page.getByRole('button', { name: /^More for / })).toHaveCount(0);
  await expect(page.getByText('Pop out')).toHaveCount(0);
});
