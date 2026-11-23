import { existsSync } from 'node:fs';
import { availableParallelism } from 'node:os';
import { defineConfig, devices } from '@playwright/test';
import { runProxyUrl } from './isolation/shared-proxy';
import { BUILD_FOLDER, BUILD_MISSING } from './synthetic/static-files';

// Not part of `bun run gates`: the tests run against the production build, which the synthetic hub serves (WP-44 E-7).
// There is no web server and no network: every request ends in the hub inside the browser context.
if (!existsSync(`${BUILD_FOLDER}index.html`)) throw new Error(`apps/client/build/ is missing: ${BUILD_MISSING}`);

// Loaded machines start the engines slowly, so the default is a few workers; `ANIMA_BROWSER_WORKERS` sets the number.
const TEST_TIMEOUT_MS = 60_000;
const EXPECT_TIMEOUT_MS = 10_000;
const DEFAULT_WORKERS = Math.min(4, Math.max(1, Math.floor(availableParallelism() / 4)));
const workers = Number.parseInt(process.env.ANIMA_BROWSER_WORKERS ?? '', 10) || DEFAULT_WORKERS;

export default defineConfig({
  globalTeardown: './isolation/global-teardown.ts',
  testDir: '.',
  testMatch: '*.spec.ts',
  outputDir: '../node_modules/.playwright-results',
  timeout: TEST_TIMEOUT_MS,
  expect: { timeout: EXPECT_TIMEOUT_MS },
  retries: 0,
  fullyParallel: true,
  workers,
  reporter: 'list',
  use: { headless: true, baseURL: 'https://hub.anima.ing' },
  // Every engine goes through its own deny-all proxy (isolation/): no test browser can reach a real host (ruling S3).
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'], proxy: { server: await runProxyUrl('chromium') } } },
    { name: 'webkit', use: { ...devices['Desktop Safari'], proxy: { server: await runProxyUrl('webkit') } } },
    { name: 'firefox', use: { ...devices['Desktop Firefox'], proxy: { server: await runProxyUrl('firefox') } } },
  ],
});
