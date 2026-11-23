import { test as base } from '@playwright/test';
import { installSyntheticHub, type SyntheticHub } from './synthetic/synthetic-hub';
import { watchPage, type PageWatch } from './page-watch';

interface Fixtures {
  /** Set per file with `test.use`. */
  signedIn: boolean;
  hub: SyntheticHub;
  watch: PageWatch;
}

/** `load` also waits for fonts and icons, which a loaded machine delays; a spec waits for an element. */
const DOCUMENT_READY = 'domcontentloaded';

/** Specs import it instead of Playwright's. */
export const test = base.extend<Fixtures>({
  page: async ({ page }, use) => {
    const goto = page.goto.bind(page);
    page.goto = (url, options) => goto(url, { waitUntil: DOCUMENT_READY, ...options });
    await use(page);
  },
  signedIn: [false, { option: true }],
  hub: [
    async ({ context, signedIn }, use) => {
      const hub = await installSyntheticHub(context);
      if (signedIn) await hub.signIn();
      await use(hub);
    },
    { auto: true },
  ],
  watch: [
    async ({ page, hub }, use) => {
      void hub;
      await use(await watchPage(page));
    },
    { auto: true },
  ],
});

export { expect } from '@playwright/test';
