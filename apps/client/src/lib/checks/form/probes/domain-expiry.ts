import type { FieldError } from '$lib/hub/hub';
import { isValidHost, normalizeHost } from '$lib/devices/form/host';
import { dayCount } from '../../days';
import type { CheckConfigInput, CheckType } from '../model/hub-types';
import { integerError, isIntegerIn, lengthError, timeoutError, toNumber } from '../model/range';

/** D-29: RDAP is slow to change, so a domain expiry check runs at most once an hour. */
const MIN_INTERVAL_SECONDS = 3600;
/** The interval proposed for a new domain expiry check (D-29). */
const PROPOSED_INTERVAL_SECONDS = 43_200;

const DOMAIN_BYTES = 253;
const WARN_DAYS: readonly [number, number] = [1, 365];

/** The stored settings of a domain expiry check, as K1 serves them. */
interface DomainExpiryStored {
  readonly domain: string;
  readonly warnDays: number;
  readonly timeoutMs: number;
}

/** What the operator typed for a domain expiry check (CHK-009). */
export interface DomainExpiryValues {
  domain: string;
  warnDays: string;
  timeoutMs: string;
  /** Whole days until the registration expires, as the hub last read it; null when unknown. Never sent. */
  daysLeft: number | null;
}

export function blankDomainExpiry(): DomainExpiryValues {
  return { domain: '', warnDays: '30', timeoutMs: '10000', daysLeft: null };
}

export function withDomainExpiryConfig(config: DomainExpiryStored, daysLeft: number | null): DomainExpiryValues {
  return { domain: config.domain, warnDays: String(config.warnDays), timeoutMs: String(config.timeoutMs), daysLeft };
}

/** D-29: a touched interval stays as the operator typed it. */
export function intervalFor(type: CheckType, current: string, ordinary: string, touched: boolean): string {
  if (touched) return current;
  return type === 'DOMAIN_EXPIRY' ? String(PROPOSED_INTERVAL_SECONDS) : ordinary;
}

/** R-102, D-29, CHK-009: a domain expiry check takes no retries, since RDAP is queried at most once an hour. */
export function takesRetries(type: CheckType): boolean {
  return type !== 'DOMAIN_EXPIRY';
}

/** D-29: refuses an interval under one hour; a malformed interval is left to the general settings check. */
export function intervalErrors(intervalSeconds: string): FieldError[] {
  if (!isIntegerIn(intervalSeconds, 0) || toNumber(intervalSeconds) >= MIN_INTERVAL_SECONDS) return [];
  return [{ field: 'intervalSeconds', message: 'Domain expiry runs at most hourly: enter 3600 seconds or more.' }];
}

function domainError(name: string): FieldError[] {
  if (name === '') return [{ field: 'domain', message: 'Enter the domain, such as example.net.' }];
  if (!isValidHost(name) || !name.includes('.') || name.includes(':') || /^[\d.]+$/.test(name)) {
    return [{ field: 'domain', message: 'Enter a domain name such as example.net, not an address.' }];
  }
  return lengthError('domain', name, DOMAIN_BYTES, 'The domain');
}

/** CHK-009, D-29. */
export function domainExpiryErrors({ domain, warnDays, timeoutMs }: DomainExpiryValues): FieldError[] {
  return [
    ...domainError(normalizeHost(domain)),
    ...integerError('warnDays', warnDays, WARN_DAYS, `Enter the warn days from ${String(WARN_DAYS[0])} to ${String(WARN_DAYS[1])}.`),
    ...timeoutError(timeoutMs),
  ];
}

export function domainExpiryInput({ domain, warnDays, timeoutMs }: DomainExpiryValues): CheckConfigInput {
  return { domainExpiry: { domain: normalizeHost(domain), warnDays: toNumber(warnDays), timeoutMs: toNumber(timeoutMs) } };
}

export function daysLeftText(daysLeft: number | null): string {
  if (daysLeft === null) return 'Days left: not known yet';
  if (daysLeft < 0) return `Expired ${dayCount(-daysLeft)} ago`;
  return `Days left: ${dayCount(daysLeft)}`;
}
