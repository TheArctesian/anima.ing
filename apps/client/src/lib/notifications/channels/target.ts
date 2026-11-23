// The list shows an origin or a host and nothing more, rebuilt from the parsed parts, whatever the hub sends (D-17, F-8).
import type { ChannelKind, ChannelSettings } from '../api';

export const TARGET_HIDDEN = 'Not shown';

const KIND_LABELS: Record<ChannelKind, string> = { WEBHOOK: 'Webhook', NTFY: 'ntfy', EMAIL: 'Email', APPRISE: 'Apprise', MOBILE: 'Mobile' };

export function kindLabel(kind: ChannelKind): string {
  return KIND_LABELS[kind];
}

function httpUrl(text: string): URL | null {
  try {
    const url = new URL(text);
    return url.protocol === 'https:' || url.protocol === 'http:' ? url : null;
  } catch {
    return null;
  }
}

export function originOf(text: string): string {
  return httpUrl(text)?.origin ?? TARGET_HIDDEN;
}

function hostOf(text: string): string {
  return httpUrl(text)?.host ?? TARGET_HIDDEN;
}

/** A DNS name, an IPv4 address, or an IPv6 address in brackets. */
const MAIL_HOST = /^(?:[a-z0-9-]+(?:\.[a-z0-9-]+)*|\[[0-9a-f:.]+\])$/i;

/** The one form the list shows and the editor compares (CR-14.1). */
export function mailHost(host: string): string | null {
  return MAIL_HOST.test(host) ? host.toLowerCase() : null;
}

/** An https origin and nothing else, `/` allowed, not even an empty query or fragment (CR-14.6, CR-14.7). */
export function cleanOrigin(text: string): string | null {
  if (/[@?#\s\\]/.test(text)) return null;
  const url = httpUrl(text);
  return url?.protocol === 'https:' && url.pathname === '/' ? url.origin : null;
}

/** What the editor shows of a server the hub holds, never its user name. */
export function shownOrigin(text: string): string {
  return httpUrl(text)?.origin ?? '';
}

/** The email target is the host and the port together (CR-14.1). */
export function mailTarget(host: string, port: number): string | null {
  const known = mailHost(host);
  return known === null ? null : `${known}:${String(port)}`;
}

/** In the form the editor compares (CR-14.1); `null` makes any target new, so the stored secrets are asked for again. */
export function storedTarget(settings: ChannelSettings): string | null {
  switch (settings.__typename) {
    case 'WebhookSettings':
      return httpUrl(settings.origin)?.origin ?? null;
    case 'NtfySettings':
    case 'AppriseSettings':
      return cleanOrigin(settings.server);
    case 'EmailSettings':
      return mailTarget(settings.host, settings.port);
  }
}

/** Email shows the server, never the recipients. */
export function targetOf(settings: ChannelSettings): string {
  switch (settings.__typename) {
    case 'WebhookSettings':
      return originOf(settings.origin);
    case 'NtfySettings':
      return hostOf(settings.server);
    case 'EmailSettings':
      return mailHost(settings.host) ?? TARGET_HIDDEN;
    case 'AppriseSettings':
      return originOf(settings.server);
  }
}
