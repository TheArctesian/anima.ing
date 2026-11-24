import { invoke, isTauri } from '@tauri-apps/api/core';
import type { ThisMachine } from '$lib/desktop/this-machine';

/** None of it is secret (HMD-044). */
export interface CloudDevice {
  readonly name: string;
  readonly computer: string;
  readonly os: ThisMachine['os'];
}

/** The answer crosses a process boundary, so every field is checked. */
function cloudDeviceFrom(answer: unknown): CloudDevice | null {
  if (typeof answer !== 'object' || answer === null) return null;
  const { name, computer, os } = answer as Record<string, unknown>;
  if (typeof name !== 'string' || name === '' || typeof computer !== 'string') return null;
  if (os !== 'MACOS' && os !== 'LINUX' && os !== 'WINDOWS') return null;
  return { name, computer, os };
}

export async function readCloudDevice(): Promise<CloudDevice | null> {
  if (!isTauri()) return null;
  try {
    return cloudDeviceFrom(await invoke('cloud_device'));
  } catch {
    return null;
  }
}
