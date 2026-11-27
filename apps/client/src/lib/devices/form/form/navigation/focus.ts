import { tick } from 'svelte';

const SECTION_TARGETS = {
  general: '[data-section="general"] button',
  ssh: '[data-section="ssh"] button',
  rdp: '[data-section="rdp"] button',
  vnc: '[data-section="vnc"] button',
  test: '[data-section="ssh"] [data-connection-test]',
} as const;

export type DeviceFormSection = keyof typeof SECTION_TARGETS;

/** Focus immediately when visible, then after Svelte opens a closed disclosure. */
export function focusSection(root: () => HTMLElement | undefined, section: DeviceFormSection): void {
  const reach = () => {
    const target = root()?.querySelector<HTMLElement>(SECTION_TARGETS[section]);
    if (target === undefined || target === null) return;
    target.focus();
    if (typeof target.scrollIntoView === 'function') target.scrollIntoView({ block: 'nearest' });
  };
  reach();
  void tick().then(reach);
}
