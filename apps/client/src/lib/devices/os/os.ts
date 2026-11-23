import type { OperatingSystem } from '$lib/hub/generated/graphql';
import type { IconName } from '$lib/icons/icons';
import { distribution as named } from './distribution';

/** Each logo has its name in words, so the logo never stands alone (UI-003, UI-015). */
const SYSTEMS: Record<OperatingSystem, { readonly icon: IconName; readonly label: string }> = {
  LINUX: { icon: 'simple-icons:linux', label: 'Linux' },
  MACOS: { icon: 'simple-icons:apple', label: 'macOS' },
  WINDOWS: { icon: 'simple-icons:windows', label: 'Windows' },
  OTHER: { icon: 'carbon:screen', label: 'Other OS' },
};

function distro(os: OperatingSystem, distribution: string | null | undefined) {
  return os === 'LINUX' ? named(distribution) : null;
}

/** The distribution's logo when the hub read one (DEV-016), else the system's. */
export function osIcon(os: OperatingSystem, distribution?: string | null): IconName {
  return distro(os, distribution)?.icon ?? SYSTEMS[os].icon;
}

export function osLabel(os: OperatingSystem, distribution?: string | null): string {
  return distro(os, distribution)?.label ?? SYSTEMS[os].label;
}
