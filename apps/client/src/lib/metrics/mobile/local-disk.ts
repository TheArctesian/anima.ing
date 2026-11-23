import { readRunnerReport } from '$lib/desktop/runner/report';
import { comparable } from '$lib/hosted/this-computer';
import type { Hub } from '$lib/hub/hub';
import { currentMobileHost } from './report';

export interface LocalDisk {
  readonly host: string;
  readonly percent: number;
}

/** The disk usage stays in this client. */
export function watchLocalDisk(hub: Hub, show: (disk: LocalDisk | undefined) => void): () => void {
  let stopped = false;
  let reading = false;

  async function refresh(): Promise<void> {
    const [host, report] = await Promise.all([currentMobileHost(hub), readRunnerReport()]);
    const total = report.totalStorageBytes ?? 0;
    const available = report.availableStorageBytes;
    if (stopped) return;
    show(host && total > 0 && available !== undefined && available >= 0 && available <= total
      ? { host: comparable(host), percent: 100 * (1 - available / total) }
      : undefined);
  }

  function read(): void {
    if (reading || document.visibilityState !== 'visible') return;
    reading = true;
    void refresh()
      .catch(() => { if (!stopped) show(undefined); })
      .finally(() => { reading = false; });
  }

  read();
  document.addEventListener('visibilitychange', read);
  return () => {
    stopped = true;
    document.removeEventListener('visibilitychange', read);
  };
}
