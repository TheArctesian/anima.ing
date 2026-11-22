import { invoke } from '@tauri-apps/api/core';

export function hasMobileReporter(origin: string, device: string): Promise<boolean> {
  return invoke<boolean>('mobile_metrics_enrolled', { origin, device });
}

export function enrolMobileReporter(origin: string, device: string, code: string): Promise<void> {
  return invoke('mobile_metrics_enrol', { origin, device, code });
}

export function pushMobileReadings(origin: string): Promise<boolean> {
  return invoke('mobile_metrics_push', { origin }).then(() => true, () => false);
}
