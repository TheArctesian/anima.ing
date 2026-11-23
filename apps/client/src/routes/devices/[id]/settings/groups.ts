import type { FieldError } from '$lib/hub/hub';
import { emptyRemoteSecrets } from '$lib/devices/form/remote/values';
import type { DeviceFormValues } from '$lib/devices/form/values';

/** A protocol and the transfer flags save whole, as the hub input takes them whole (D17). */
export type SettingsGroup = 'name' | 'host' | 'kind' | 'os' | 'tags' | 'macAddress' | 'metricsSource' | 'ssh' | 'rdp' | 'vnc' | 'transfer';

export type SettingKey = SettingsGroup | `${'ssh' | 'rdp' | 'vnc' | 'transfer'}.${string}`;

export function groupOf(key: SettingKey): SettingsGroup {
  return key.split('.')[0] as SettingsGroup;
}

export function inGroup(field: string, group: SettingsGroup): boolean {
  if (group === 'ssh' && field.startsWith('credential.')) return true;
  return field === group || field.startsWith(`${group}.`);
}

/** The saved values with `group` as shown, and no secrets: what one change sends (D17, D-2). */
export function withGroup(saved: DeviceFormValues, shown: DeviceFormValues, group: SettingsGroup): DeviceFormValues {
  return {
    ...saved,
    [group]: structuredClone(shown[group]),
    credential: { password: '', privateKey: '', passphrase: '' },
    remoteSecrets: emptyRemoteSecrets(),
  };
}

export function sameGroup(a: DeviceFormValues, b: DeviceFormValues, group: SettingsGroup): boolean {
  return JSON.stringify(a[group]) === JSON.stringify(b[group]);
}

export function valueAt(values: DeviceFormValues, key: SettingKey): unknown {
  const [group, field] = key.split('.') as [SettingsGroup, string | undefined];
  const top: unknown = values[group];
  return field === undefined ? top : (top as Record<string, unknown>)[field];
}

export function setValueAt(values: DeviceFormValues, key: SettingKey, value: unknown): void {
  const [group, field] = key.split('.') as [SettingsGroup, string | undefined];
  if (field === undefined) (values as unknown as Record<string, unknown>)[group] = structuredClone(value);
  else (values[group] as unknown as Record<string, unknown>)[field] = structuredClone(value);
}

export function replaceGroup(errors: readonly FieldError[], group: SettingsGroup, next: readonly FieldError[]): FieldError[] {
  return [...errors.filter((error) => !inGroup(error.field, group)), ...next];
}
