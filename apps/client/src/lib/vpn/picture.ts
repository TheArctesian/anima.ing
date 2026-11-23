// VPN-009 to VPN-011, UI-003.
import type { KeyState, SiteRouteState, VpnPath } from '$lib/hub/generated/graphql';
import type { BrokerState, VpnNode } from './api';

const DAY_MS = 86_400_000;

export function pathLabel(path: VpnPath, region: string | null): string {
  switch (path) {
    case 'DIRECT':
      return 'Direct';
    case 'PEER_RELAY':
      return 'Peer relay';
    case 'RELAYED':
      return region === null || region === '' ? 'Relayed' : `Relayed (${region})`;
    case 'UNKNOWN':
      return 'Not known';
  }
}

export function stateLabel(state: SiteRouteState): string {
  switch (state) {
    case 'SERVING':
      return 'Serving';
    case 'OFFLINE':
      return 'Router offline';
    case 'NO_ROUTER':
      return 'No router';
  }
}

const plural = (count: number, unit: string): string => `${String(count)} ${unit}${count === 1 ? '' : 's'}`;

function dateText(iso: string, locale?: string): string {
  const at = new Date(iso);
  return Number.isNaN(at.getTime()) ? 'an unknown date' : new Intl.DateTimeFormat(locale, { dateStyle: 'medium' }).format(at);
}

export function keyText(node: Pick<VpnNode, 'keyState' | 'keyExpiry'>, now: Date, locale?: string): string {
  const { keyState, keyExpiry } = node;
  if (keyState === 'NEVER' || keyExpiry === null) return keyState === 'NEVER' ? 'Does not expire' : 'Expiry not known';
  const days = (Date.parse(keyExpiry) - now.getTime()) / DAY_MS;
  switch (keyState) {
    case 'VALID':
      return `Expires on ${dateText(keyExpiry, locale)}`;
    case 'EXPIRING':
      return `Expires in ${plural(Math.max(1, Math.ceil(days)), 'day')}`;
    case 'EXPIRED': {
      const ago = Math.floor(-days);
      return ago < 1 ? 'Expired today' : `Expired ${plural(ago, 'day')} ago`;
    }
    case 'TAGGED_WITH_EXPIRY':
      return `Tagged but expires on ${dateText(keyExpiry, locale)}: renew as a tagged node`;
  }
}

/** A word always sits beside the tone (UI-003). */
export function keyTone(state: KeyState): 'up' | 'warning' | 'down' {
  switch (state) {
    case 'NEVER':
    case 'VALID':
      return 'up';
    case 'EXPIRING':
    case 'TAGGED_WITH_EXPIRY':
      return 'warning';
    case 'EXPIRED':
      return 'down';
  }
}

/** The broker can renew it (BD-17). */
export function renewable(node: Pick<VpnNode, 'role' | 'tags' | 'keyState'>): boolean {
  return node.role === 'CLIENT' && node.tags.length === 0 && (node.keyState === 'EXPIRING' || node.keyState === 'EXPIRED');
}

const needsAttention = (state: KeyState): boolean => state !== 'NEVER' && state !== 'VALID';

export function sortedNodes(nodes: readonly VpnNode[]): readonly VpnNode[] {
  const rank = (node: VpnNode): number => (node.inAnima ? 2 : 0) + (needsAttention(node.keyState) ? 0 : 1);
  return [...nodes].sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name));
}

export function readAgo(readAt: string, now: Date): string {
  const seconds = Math.max(0, Math.floor((now.getTime() - Date.parse(readAt)) / 1000));
  if (Number.isNaN(seconds)) return 'Read at an unknown time';
  if (seconds < 90) return `Read ${String(seconds)} s ago`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 90) return `Read ${String(minutes)} min ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 48) return `Read ${String(hours)} h ago`;
  return `Read ${plural(Math.round(hours / 24), 'day')} ago`;
}

/** Why the actions that need the broker are off (VP-7). */
export function brokerReason(state: BrokerState): string | null {
  switch (state) {
    case 'READY':
      return null;
    case 'NOT_SET':
      return 'Enrolment needs the broker (ANIMA_VPN_BROKER).';
    case 'NOT_INSTALLED':
      return 'The broker is not installed on the VPS (vpn.md Section 11).';
    case 'FAILED':
      return 'The hub could not reach the broker. Retry in a minute.';
  }
}
