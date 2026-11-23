import type { FieldError } from '$lib/hub/hub';
import { normalizeHost } from './host';

/** A site Add device creates with the device when the hub has none yet (VPN-001, VPN-002). */
export interface NewSite {
  name: string;
  subnet: string;
}

export type SitePlacement = { readonly kind: 'existing'; readonly id: string } | { readonly kind: 'new'; readonly site: NewSite };

/** The VPN address range (NET-001): computers reached by their VPN address share one site. */
export const VPN_SITE: NewSite = { name: 'VPN', subnet: '100.64.0.0/10' };

/** The address range of the built-in link (HMD-044): the computers of a hosted hub share one site. */
export const LINK_SITE: NewSite = { name: 'Link', subnet: 'fd41:4e49:4d41::/48' };

const EMPTY: NewSite = { name: '', subnet: '' };

function octets(text: string): number[] | undefined {
  const parts = text.split('.');
  if (parts.length !== 4 || !parts.every((part) => /^(0|[1-9]\d{0,2})$/.test(part))) return undefined;
  const numbers = parts.map(Number);
  return numbers.every((number) => number <= 255) ? numbers : undefined;
}

function toNumber(parts: readonly number[]): number {
  return parts.reduce((total, part) => total * 256 + part, 0);
}

function network(address: number, length: number): number {
  const size = 2 ** (32 - length);
  return Math.floor(address / size) * size;
}

export function suggestedSite(host: string): NewSite {
  const parts = octets(normalizeHost(host));
  if (parts === undefined) return { ...EMPTY };
  if (isVpnAddress(host)) return { ...VPN_SITE };
  return { name: 'Local network', subnet: `${parts.slice(0, 3).join('.')}.0/24` };
}

export function isVpnAddress(host: string): boolean {
  const parts = octets(normalizeHost(host));
  return parts !== undefined && network(toNumber(parts), 10) === toNumber([100, 64, 0, 0]);
}

/** IPv6 blocks are left to the hub. */
function subnetProblem(subnet: string): string | undefined {
  const [address = '', length, extra] = subnet.split('/');
  if (length === undefined || extra !== undefined || !/^\d{1,3}$/.test(length)) return 'Enter a subnet such as 10.10.0.0/24.';
  if (address.includes(':')) return Number(length) <= 128 ? undefined : 'Enter a prefix length up to 128.';
  const parts = octets(address);
  if (parts === undefined) return 'Enter a subnet such as 10.10.0.0/24.';
  const prefix = Number(length);
  if (prefix > 32) return 'Enter a prefix length up to 32.';
  const value = toNumber(parts);
  return network(value, prefix) === value ? undefined : 'Use the first address of the subnet.';
}

export function validateNewSite(site: NewSite): FieldError[] {
  const errors: FieldError[] = [];
  if (site.name.trim() === '') errors.push({ field: 'site.name', message: 'Enter a site name.' });
  const problem = subnetProblem(site.subnet.trim());
  if (problem !== undefined) errors.push({ field: 'site.subnet', message: problem });
  return errors;
}

/** Moved under `site.` so a hub error shows beside the site fields, not on the device's name. */
export function siteFieldErrors(fields: readonly FieldError[]): FieldError[] {
  return fields.map((error) => ({ ...error, field: `site.${error.field}` }));
}
