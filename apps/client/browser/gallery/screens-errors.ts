import { expect } from '@playwright/test';
import type { Fixtures } from '../synthetic/fixtures';
import { HUB_ORIGIN } from '../synthetic/access';
import type { Screen } from './shots';
import { DEVICE_IDS, failOperations, onOperations, openDevice } from './steps';

// The failure screens come last, so the numbers of the screens before them stay.

/** Projects is an extension that the synthetic hub has off. */
const projectsOn = (loaded: Fixtures): Fixtures => ({ ...loaded, extensionsOn: ['projects'] });

export const ERROR_SCREENS: readonly Screen[] = [
  {
    name: 'page-not-found',
    open: async (page) => {
      await page.goto(`${HUB_ORIGIN}/no-such-page`);
      await page.waitForTimeout(1500);
    },
  },
  {
    name: 'project-not-found',
    fixtures: projectsOn,
    open: async (page) => {
      await page.goto(`${HUB_ORIGIN}/projects/no-such-project`);
      await page.waitForTimeout(1500);
    },
  },
  {
    name: 'projects-hub-error',
    fixtures: projectsOn,
    open: async (page) => {
      await failOperations(page, ['Projects']);
      await page.goto(`${HUB_ORIGIN}/projects`);
      await page.waitForTimeout(1500);
    },
  },
  {
    name: 'agent-usage-hub-error',
    open: async (page) => {
      await failOperations(page, ['AgentUsage']);
      await page.goto(`${HUB_ORIGIN}/agents/usage`);
      await page.waitForTimeout(1500);
    },
  },
  {
    name: 'extensions-hub-error',
    open: async (page) => {
      await failOperations(page, ['Extensions']);
      await page.goto(`${HUB_ORIGIN}/extensions`);
      await page.waitForTimeout(1500);
    },
  },
  {
    name: 'containers-hub-error',
    open: async (page) => {
      await failOperations(page, ['Containers']);
      await openDevice(page, DEVICE_IDS.gpuBox, 'gpu-box');
      const tab = page.getByRole('tab', { name: 'Containers' });
      if (await tab.count()) await tab.click();
      await expect(page.getByRole('main')).toBeVisible();
      await page.waitForTimeout(1500);
    },
  },
  {
    // PRO-059: a hub from before project groups does not know the field.
    name: 'projects-older-hub',
    fixtures: projectsOn,
    open: async (page) => {
      const body = JSON.stringify({ data: null, errors: [{ message: 'Unknown field "projectGroups" on type "Query".' }] });
      await onOperations(page, ['SharedProjectGroups'], (route) => route.fulfill({ status: 200, contentType: 'application/graphql-response+json', body }));
      await page.goto(`${HUB_ORIGIN}/projects`);
      await expect(page.getByText('Groups need a newer hub')).toBeVisible();
      await page.waitForTimeout(1000);
    },
  },
];
