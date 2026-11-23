import type { OperatingSystem, Protocol, Status } from '$lib/hub/generated/graphql';

/** The client's own name for one open tab; a tab outlives the sessions started in it (D-5). */
export type TabId = string;

export type SessionProtocol = Protocol;

/** Where on screen the control that opened a tab was, for the open motion (MOT-005). */
export interface Origin {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}

/** One session tab in the strip (RA-001). `sessionId` is the hub session once one has started. */
export interface ProtocolTab {
  readonly id: TabId;
  readonly deviceId: string;
  readonly deviceName: string;
  readonly os?: OperatingSystem;
  /** The Linux distribution's os-release ID, for its logo on the tab (DEV-016); absent when not known. */
  readonly distribution?: string;
  readonly protocol: SessionProtocol;
  readonly origin: Origin | null;
  readonly sessionId: string | null;
  /** The window title the remote shell set, already cleaned (K4 §8.7); absent until it sets one. */
  readonly title?: string;
}

/** The web interface a web tab shows (K1 `WebInterface`); the tab reads and compares its pin (WEB-008). */
export interface WebTarget {
  readonly id: string;
  readonly deviceId: string;
  readonly url: string;
  readonly label: string;
  readonly pinnedFingerprintSha256: string | null;
}

/** A tab showing one of the device's web interfaces (WEB-003); it starts no hub session. */
export interface WebPageTab {
  readonly id: TabId;
  readonly deviceId: string;
  readonly deviceName: string;
  readonly os?: OperatingSystem;
  readonly distribution?: string;
  readonly protocol: 'WEB';
  readonly origin: Origin | null;
  readonly sessionId: null;
  readonly title?: string;
  readonly web: WebTarget;
  /** The device's status when the tab opened; the page shows Down for a device that is down (WEB-010). */
  readonly status: Status;
}

export type Tab = ProtocolTab | WebPageTab;

/** The open tabs in strip order and the selected one; `null` shows the pages (D-11). */
export interface Tabs {
  readonly tabs: readonly Tab[];
  readonly activeId: TabId | null;
}

export const noTabs: Tabs = { tabs: [], activeId: null };

/** `tab` is added at the end and selected. A device may have several tabs (RA-001). */
export function opened(state: Tabs, tab: Tab): Tabs {
  return { tabs: [...state.tabs, tab], activeId: tab.id };
}

export function selected(state: Tabs, id: TabId): Tabs {
  return state.tabs.some((tab) => tab.id === id) ? { ...state, activeId: id } : state;
}

export function deselected(state: Tabs): Tabs {
  return { ...state, activeId: null };
}

export function closed(state: Tabs, id: TabId): Tabs {
  const index = state.tabs.findIndex((tab) => tab.id === id);
  if (index === -1) return state;
  const tabs = state.tabs.filter((tab) => tab.id !== id);
  if (state.activeId !== id) return { tabs, activeId: state.activeId };
  const neighbour = tabs[Math.min(index, tabs.length - 1)];
  return { tabs, activeId: neighbour?.id ?? null };
}

function clampIndex(index: number, length: number): number {
  return Math.min(Math.max(Math.trunc(index), 0), length - 1);
}

export function moved(state: Tabs, from: number, to: number): Tabs {
  if (state.tabs.length === 0) return state;
  const start = clampIndex(from, state.tabs.length);
  const end = clampIndex(to, state.tabs.length);
  if (start === end) return state;
  const tabs = [...state.tabs];
  const [tab] = tabs.splice(start, 1);
  if (tab === undefined) return state;
  tabs.splice(end, 0, tab);
  return { ...state, tabs };
}

export function withSession(state: Tabs, id: TabId, sessionId: string | null): Tabs {
  return { ...state, tabs: state.tabs.map((tab) => (tab.id === id && tab.protocol !== 'WEB' ? { ...tab, sessionId } : tab)) };
}

export function titled(state: Tabs, id: TabId, title: string): Tabs {
  return { ...state, tabs: state.tabs.map((tab) => (tab.id === id ? { ...tab, title } : tab)) };
}
