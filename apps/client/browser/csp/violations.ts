import { expect, type BrowserContext, type Page, type Request } from '@playwright/test';
import { withoutQuery, type CspViolation, type PageWatch } from '../page-watch';
import { HUB_ORIGIN } from '../synthetic/access';
import { untilClosed } from '../synthetic/closed';
import { staticSite } from '../synthetic/static-files';

// A Report-Only copy of the policy is the second channel: it needs no page script, so it also sees violations during boot.
// Firefox sends CSP reports past `context.route`, so the report host is `.invalid` (RFC 6761) and no report leaves the machine.

const REPORT_HOST = 'csp-report.invalid';
const SENTINEL = 'https://csp-sentinel.invalid/';

export interface PolicyReports {
  readonly reports: readonly CspViolation[];
}

export interface Seen {
  readonly events: readonly CspViolation[];
  readonly reports: readonly CspViolation[];
}

/** The rule and what it refused, from a CSP report body. Never the document URL or the script sample. */
function reportOf(request: Request): CspViolation {
  let body: unknown = null;
  try {
    body = request.postDataJSON();
  } catch {
    // A body that is not JSON is still a report; its fields stay unknown.
  }
  const report = ((body as { 'csp-report'?: Record<string, unknown> } | null)?.['csp-report'] ?? {}) as Record<string, unknown>;
  const field = (name: string): string => (typeof report[name] === 'string' ? report[name] : '');
  return {
    effectiveDirective: field('effective-directive') || field('violated-directive') || 'unknown',
    // Only the Report-Only copy names a report URL, so every report is of that copy.
    disposition: 'report',
    blockedUri: withoutQuery(field('blocked-uri')),
    sourceFile: withoutQuery(field('source-file')),
  };
}

/** Call before the first navigation. */
export async function servePolicy(context: BrowserContext, policy: string): Promise<PolicyReports> {
  const site = staticSite();
  const reports: CspViolation[] = [];
  context.on('request', (request) => {
    if (new URL(request.url()).hostname === REPORT_HOST) reports.push(reportOf(request));
  });
  await context.route('**/*', (route) =>
    untilClosed(async () => {
      const request = route.request();
      const url = new URL(request.url());
      if (url.hostname === REPORT_HOST) {
        await route.fulfill({ status: 204, body: '' });
      } else if (request.resourceType() === 'document' && url.origin === HUB_ORIGIN) {
        const answer = site.answer(url.pathname);
        const reportOnly = `${policy}; report-uri https://${REPORT_HOST}/`;
        const headers = {
          ...answer.headers,
          'content-security-policy': policy,
          'content-security-policy-report-only': reportOnly,
        };
        await route.fulfill({
          status: answer.status,
          headers,
          body: answer.body,
        });
      } else {
        await route.fallback();
      }
    }, context),
  );
  return { reports };
}

const isSentinel = (violation: CspViolation): boolean => violation.blockedUri.includes('csp-sentinel.invalid');

/** A sentinel violation reaches both channels first, so a dead channel or a report in flight cannot pass for "none". */
export async function violationsSeen(page: Page, watch: PageWatch, policy: PolicyReports): Promise<Seen> {
  await page.evaluate(async (url) => {
    // eslint-disable-next-line no-restricted-globals -- test code in the page: the sentinel request the policy must refuse
    await fetch(url).catch(() => undefined);
  }, SENTINEL);
  await expect
    .poll(() => ({
      event: watch.cspViolations.some(isSentinel),
      report: policy.reports.some(isSentinel),
    }))
    .toEqual({ event: true, report: true });
  return violationsSoFar(watch, policy);
}

export function violationsSoFar(watch: PageWatch, policy: PolicyReports): Seen {
  const real = (violations: readonly CspViolation[]): CspViolation[] => violations.filter((violation) => !isSentinel(violation));
  return { events: real(watch.cspViolations), reports: real(policy.reports) };
}

export async function expectNoViolations(page: Page, watch: PageWatch, policy: PolicyReports): Promise<void> {
  expect(await violationsSeen(page, watch, policy)).toEqual({
    events: [],
    reports: [],
  });
}
