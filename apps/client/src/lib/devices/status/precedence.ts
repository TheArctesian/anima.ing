import type { DeviceStatus } from '../card/card';

/** The one status a device shows (DEV-006): the hub's five, and Warning, which only the precedence below produces. */
export type CombinedStatus = DeviceStatus | 'WARNING';

export interface CheckState {
  readonly name: string;
  readonly status: DeviceStatus;
  readonly paused: boolean;
}

export type SecurityWarning = 'HOST_KEY_CHANGED' | 'CERTIFICATE_CHANGED';

export interface Expiry {
  readonly kind: 'CERTIFICATE' | 'DOMAIN';
  readonly name: string;
  readonly daysLeft: number;
}

export interface StatusFacts {
  /** The device is inside a maintenance window, or its manual flag is set. */
  readonly maintenance: boolean;
  readonly checks: readonly CheckState[];
  readonly securityWarnings: readonly SecurityWarning[];
  readonly expiries: readonly Expiry[];
  /** The shortest NOT-006 threshold in days (default 7): an expiry fewer days away is a warning. */
  readonly warnWithinDays: number;
  readonly metricsOverThreshold: readonly string[];
}

export interface StatusReason {
  readonly status: CombinedStatus;
  readonly text: string;
}

export interface CombinedResult {
  readonly status: CombinedStatus;
  readonly reasons: readonly StatusReason[];
}

const WARNING_TEXT: Record<SecurityWarning, string> = {
  HOST_KEY_CHANGED: 'SSH host key changed',
  CERTIFICATE_CHANGED: 'Certificate changed',
};

function enabled(facts: StatusFacts): readonly CheckState[] {
  return facts.checks.filter((check) => !check.paused);
}

function reasons(status: CombinedStatus, texts: readonly string[]): StatusReason[] {
  return texts.map((text) => ({ status, text }));
}

function days(count: number): string {
  if (count < 0) return 'has expired';
  return count === 1 ? 'expires in 1 day' : `expires in ${String(count)} days`;
}

function maintenanceReasons(facts: StatusFacts): StatusReason[] {
  return reasons('MAINTENANCE', facts.maintenance ? ['In a maintenance window'] : []);
}

function downReasons(facts: StatusFacts): StatusReason[] {
  return reasons('DOWN', enabled(facts).filter((check) => check.status === 'DOWN').map((check) => `${check.name} is down`));
}

function warningReasons(facts: StatusFacts): StatusReason[] {
  const soon = facts.expiries.filter((expiry) => expiry.daysLeft < facts.warnWithinDays);
  const label = (expiry: Expiry): string => `${expiry.kind === 'CERTIFICATE' ? 'Certificate' : 'Domain'} ${expiry.name} ${days(expiry.daysLeft)}`;
  return reasons('WARNING', [...facts.securityWarnings.map((warning) => WARNING_TEXT[warning]), ...soon.map(label)]);
}

function degradedReasons(facts: StatusFacts): StatusReason[] {
  const slow = enabled(facts).filter((check) => check.status === 'DEGRADED').map((check) => `${check.name} is degraded`);
  return reasons('DEGRADED', [...slow, ...facts.metricsOverThreshold.map((metric) => `${metric} over threshold`)]);
}

function restingReason(facts: StatusFacts): StatusReason {
  const checks = enabled(facts);
  if (checks.some((check) => check.status === 'UP')) return { status: 'UP', text: 'At least one check is up' };
  const text = checks.length === 0 && facts.checks.length > 0 ? 'All checks paused' : 'No results yet';
  return { status: 'UNKNOWN', text };
}

/** DEV-006: Maintenance, Down, Warning, Degraded, Up, Unknown; the first that applies wins. */
export function combinedStatus(facts: StatusFacts): CombinedResult {
  const applied = [maintenanceReasons, downReasons, warningReasons, degradedReasons].flatMap((rule) => rule(facts));
  const [first] = applied;
  if (first === undefined) {
    const resting = restingReason(facts);
    return { status: resting.status, reasons: [resting] };
  }
  return { status: first.status, reasons: applied };
}
