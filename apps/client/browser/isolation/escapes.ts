import type { Attempt } from './deny-proxy';

// Refused proxy attempts that the harness itself explains (WP-44 ruling S3).

export const HARNESS_TARGETS: readonly string[] = [
  // Firefox sends every CSP report past `context.route`, straight to the network (csp/violations.ts).
  'csp-report.invalid:443',
  // The end-of-test marker of csp/violations.ts, when an engine tries it before the policy refuses it.
  'csp-sentinel.invalid:443',
  'isolation-probe.invalid:443',
  'isolation-probe.invalid:80',
];

/** WebKit connects to a fixture tab's address before `context.route` answers it; `.example` never resolves (RFC 2606). */
const FIXTURE_HOST_SUFFIX = '.annex.example';

/** WebKit opens a TLS connection to the page's origin when a navigation starts, before `context.route` can answer it. */
const ENGINE_TARGETS: Readonly<Record<string, readonly string[]>> = { webkit: ['hub.anima.ing:443'] };

export function escapedAttempts(engine: string, attempts: readonly Attempt[]): Attempt[] {
  const explained = [...HARNESS_TARGETS, ...(ENGINE_TARGETS[engine] ?? [])];
  const isFixtureHost = (target: string): boolean => target.slice(0, target.lastIndexOf(':')).endsWith(FIXTURE_HOST_SUFFIX);
  return attempts.filter((attempt) => !explained.includes(attempt.target) && !isFixtureHost(attempt.target));
}
