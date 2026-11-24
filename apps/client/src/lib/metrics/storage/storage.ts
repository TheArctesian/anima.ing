import type { DriveKind } from '$lib/hub/generated/graphql';
import type { NewMetrics, StorageDrive } from '../api';
import { DEGRADED_ABOVE, formatBytes, formatBytesPerSecond, formatPercent } from '../units';

export interface MountView {
  readonly path: string;
  readonly percent: number | null;
  /** The size is the row's own column. */
  readonly text: string;
  readonly spoken: string;
  readonly degraded: boolean;
}

/** One row of the Storage section: a drive, or a mount on no drive the hub knows (MET-029, MET-031). */
export interface StorageRow {
  readonly key: string;
  readonly title: string;
  /** A word, never a colour alone (MET-027). */
  readonly kind: string | null;
  readonly size: string | null;
  readonly mounted: boolean;
  readonly mounts: readonly MountView[];
  readonly showPaths: boolean;
  readonly activity: string | null;
}

const KIND_TEXT: Readonly<Record<DriveKind, string>> = { NVME: 'NVMe', SSD: 'SSD', HDD: 'HDD', DISK: 'Disk' };

function plural(count: number, one: string, many: string): string {
  return `${String(count)} ${count === 1 ? one : many}`;
}

export function summaryText(drives: readonly StorageDrive[]): string {
  const sizes = drives.flatMap((drive) => (drive.sizeBytes === null ? [] : [drive.sizeBytes]));
  const unmounted = drives.filter((drive) => !drive.mounted).length;
  const parts = [plural(drives.length, 'drive', 'drives')];
  if (sizes.length > 0) parts.push(formatBytes(sizes.reduce((sum, size) => sum + size, 0)));
  if (unmounted > 0) parts.push(`${String(unmounted)} not mounted`);
  return parts.join(' · ');
}

function mountView(mount: StorageDrive['mounts'][number]): MountView {
  const { percent, sizeBytes, usedBytes } = mount;
  const shown = percent === null ? null : Math.min(100, Math.max(0, percent));
  const free = sizeBytes !== null && usedBytes !== null && sizeBytes >= usedBytes ? `${formatBytes(sizeBytes - usedBytes)} free` : null;
  const used = usedBytes === null ? null : `${formatBytes(usedBytes)} used${sizeBytes === null ? '' : ` of ${formatBytes(sizeBytes)}`}`;
  const text = [used, free, shown === null ? null : formatPercent(shown)].filter((part) => part !== null).join(' · ');
  const spoken = [mount.path, used, free, shown === null ? 'no reading' : formatPercent(shown)].filter((part) => part !== null).join(', ');
  return { path: mount.path, percent: shown, text: text === '' ? '—' : text, spoken, degraded: shown !== null && shown > DEGRADED_ABOVE };
}

function activityText(drive: StorageDrive): string | null {
  const parts = [
    drive.readBps === null ? null : `Read ${formatBytesPerSecond(drive.readBps)}`,
    drive.writeBps === null ? null : `Write ${formatBytesPerSecond(drive.writeBps)}`,
    drive.busyPercent === null ? null : `Busy ${formatPercent(drive.busyPercent)}`,
  ].filter((part) => part !== null);
  return parts.length === 0 ? null : parts.join(' · ');
}

/** Mount label used by the retired iOS reporter that sent raw filesystem block counts. */
export const LEGACY_IOS_DISK_MOUNT = 'App data';

/** The rows in the hub's order: mounted drives, lone mounts, then drives that are not mounted (MET-029). */
export function storageRows(drives: readonly StorageDrive[]): readonly StorageRow[] {
  return drives.filter((drive) => drive.name !== null || drive.mounts[0]?.path !== LEGACY_IOS_DISK_MOUNT).map((drive) => {
    const lone = drive.name === null;
    const title = drive.name ?? drive.mounts[0]?.path ?? '—';
    return {
      key: `${lone ? 'mount' : 'drive'}:${title}`,
      title,
      kind: drive.kind === null ? null : KIND_TEXT[drive.kind],
      size: drive.sizeBytes === null ? null : formatBytes(drive.sizeBytes),
      mounted: drive.mounted,
      mounts: drive.mounts.map(mountView),
      showPaths: !lone,
      activity: activityText(drive),
    };
  });
}

function withSample(drive: StorageDrive, sample: NewMetrics['samples'][number]): StorageDrive {
  const { kind, label, value } = sample;
  if (kind === 'DISK_PERCENT' || kind === 'DISK_BYTES') {
    if (!drive.mounts.some((mount) => mount.path === label)) return drive;
    const field = kind === 'DISK_PERCENT' ? 'percent' : 'usedBytes';
    return { ...drive, mounts: drive.mounts.map((mount) => (mount.path === label ? { ...mount, [field]: value } : mount)) };
  }
  if (drive.name !== label) return drive;
  if (kind === 'DISK_READ_BPS') return { ...drive, readBps: value };
  if (kind === 'DISK_WRITE_BPS') return { ...drive, writeBps: value };
  if (kind === 'DISK_BUSY_PERCENT') return { ...drive, busyPercent: value };
  return drive;
}

/** MET-028: the drive list itself changes only with a reload. */
export function mergeStorage(drives: readonly StorageDrive[], added: NewMetrics): readonly StorageDrive[] {
  return drives.map((drive) => added.samples.reduce(withSample, drive));
}
