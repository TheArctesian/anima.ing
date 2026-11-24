import type { UpdaterPort } from '$lib/desktop/updater';
import { warnToProcessLog } from '$lib/desktop/process-log';
import { capabilities } from '$lib/platform/platform';
import { isLocalBuild } from './local-build';
import { provideUpdates, Updates } from './updates.svelte';

/** The shell's updater, loaded by the first call that needs it: a client that never calls it never loads it (UPD-001). */
const load = () => import('$lib/desktop/updater');
const lazyUpdater: UpdaterPort = {
  check: async () => (await load()).desktopUpdater.check(),
  restart: async () => (await load()).desktopUpdater.restart(),
  currentVersion: async () => (await load()).desktopUpdater.currentVersion(),
};

/** Only a release build checks: dev shells and trial bundles have no updater plugin (UPD-001, DEF-001). */
export function provideDesktopUpdates(): void {
  const updates = new Updates(lazyUpdater, { warn: warnToProcessLog });
  provideUpdates(updates);
  $effect(() => (capabilities().updates && !import.meta.env.DEV && !isLocalBuild() ? updates.start() : undefined));
}
