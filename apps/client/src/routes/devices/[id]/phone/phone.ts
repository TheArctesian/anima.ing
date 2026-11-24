import type { DeviceCheck } from '$lib/checks/api';
import type { Beat } from '$lib/checks/heartbeats';
import { statusLabel } from '$lib/devices/card/card';

const NONE = '—';

export function responseText(beats: readonly Beat[]): string {
  const timed = beats.findLast((beat) => beat.responseTimeMs != null);
  return timed?.responseTimeMs == null ? NONE : `${String(timed.responseTimeMs)} ms`;
}

export function checkValue(check: Pick<DeviceCheck, 'status' | 'certificateDaysLeft' | 'domainDaysLeft' | 'lastHeartbeat'>): string {
  const days = check.certificateDaysLeft ?? check.domainDaysLeft;
  if (days != null) return `${String(days)} d`;
  const ms = check.lastHeartbeat?.responseTimeMs;
  return ms == null ? statusLabel(check.status) : `${String(ms)} ms`;
}
