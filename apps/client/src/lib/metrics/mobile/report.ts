import { readLink } from '$lib/desktop/link/shell';
import { activeLinkKey } from '$lib/devices/enrollment/enrol-linked-device';
import { linkApi } from '$lib/link/api';
import { enrolMobileReporter, hasMobileReporter, pushMobileReadings } from '$lib/desktop/metrics/mobile';
import { devicesApi } from '$lib/devices/client';
import { comparable } from '$lib/hosted/this-computer';
import type { Hub } from '$lib/hub/hub';
import { metricsApi } from '$lib/metrics/api';

const REPORT_INTERVAL = 20_000;
const RECOVERY_INTERVAL = 120_000;
const enabling = new Map<string, Promise<boolean>>();

export function enableMobileMetrics(hub: Hub, host: string): Promise<boolean> {
  const key = `${hub.address.origin}/${comparable(host)}`;
  const pending = enabling.get(key);
  if (pending !== undefined) return pending;
  const task = enable(hub, host).catch(() => false).finally(() => enabling.delete(key));
  enabling.set(key, task);
  return task;
}

async function enable(hub: Hub, host: string): Promise<boolean> {
  const listed = await devicesApi(hub).deviceHosts();
  if (!listed.ok) return false;
  const matches = listed.value.filter((device) => comparable(device.host) === comparable(host));
  const [device] = matches;
  if (matches.length !== 1 || device === undefined) return false;
  const origin = hub.address.origin;
  if (await hasMobileReporter(origin, device.id)) return pushMobileReadings(origin);
  // An existing reporter owns its token. Enrolment would replace it.
  if (device.metricsSource !== 'NONE') return false;
  const enrolment = await metricsApi(hub).enrolDaemon(device.id);
  if (!enrolment.ok) return false;
  await enrolMobileReporter(origin, device.id, enrolment.value.code);
  return pushMobileReadings(origin);
}

/** A device enrolled before reporting was supported may have a row but no native credential yet. */
async function recover(hub: Hub): Promise<void> {
  const local = await readLink();
  if (!local.ok) return;
  const key = activeLinkKey(hub.address.origin, local.value);
  if (key === undefined) return;
  const linked = await linkApi(hub).devices();
  if (!linked.ok) return;
  const device = linked.value.find((entry) => entry.key === key);
  if (device?.address) await enableMobileMetrics(hub, device.address);
}

export function watchMobileMetrics(hub: Hub): () => void {
  let inFlight = false;
  let stopped = false;
  let nextRecovery = 0;
  async function send(): Promise<void> {
    if (await pushMobileReadings(hub.address.origin) || stopped || Date.now() < nextRecovery) return;
    nextRecovery = Date.now() + RECOVERY_INTERVAL;
    await recover(hub);
  }
  function report(): void {
    if (document.visibilityState !== 'visible' || inFlight) return;
    inFlight = true;
    void send().catch(() => undefined).finally(() => { inFlight = false; });
  }
  const timer = window.setInterval(report, REPORT_INTERVAL);
  document.addEventListener('visibilitychange', report);
  report();
  return () => {
    stopped = true;
    window.clearInterval(timer);
    document.removeEventListener('visibilitychange', report);
  };
}
