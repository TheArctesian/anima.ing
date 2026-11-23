import type { ApplicationRouteWait } from '$lib/hub/generated/graphql';
import type { ApplicationRoute } from './api';
import { certificateStatus, isStream, proofStatus, type Status } from './route';

/** One step that a route waits for, in the order the hub checks them (ApplicationRouteWait). */
export interface RouteCheck extends Status {
  readonly title: string;
}

/** One DNS record that the operator adds; `value` is null when the hub does not give it. */
export interface DnsRecord {
  readonly type: 'TXT' | 'CNAME' | 'A';
  readonly name: string;
  readonly valueLabel: string;
  readonly value: string | null;
}

const ORDER: readonly ApplicationRouteWait[] = ['PROOF', 'MACHINE_CONSENT', 'APPLICATION_RECORD', 'CERTIFICATE'];

/** The place of the step the route waits for; past the last step when it serves. */
function waitsAt(route: Pick<ApplicationRoute, 'waitsFor'>): number {
  return route.waitsFor === null ? ORDER.length : ORDER.indexOf(route.waitsFor);
}

/** The hub keeps no field for the record of the application: its state follows from the step the route waits for. */
function recordCheck(route: ApplicationRoute): RouteCheck {
  const at = waitsAt(route);
  const title = 'Record of the application';
  if (at > 2) return { title, label: 'Recorded', tone: 'up', detail: 'The hub knows the application on this device and port.' };
  if (at === 2) return { title, label: 'Waits', tone: 'unknown', detail: 'The hub keeps no record of an application and its ports yet (task H6).' };
  return { title, label: 'Not checked yet', tone: 'unknown', detail: 'The hub checks it after the steps above.' };
}

/** The checks of one route, in the order the hub checks them. A stream has no certificate of the hub. */
export function routeChecks(route: ApplicationRoute, device: string, now: Date, locale?: string): RouteCheck[] {
  const proof = proofStatus(route);
  const consent: RouteCheck = route.machineConsent
    ? { title: 'Consent at the machine', label: 'Given', tone: 'up', detail: `${device} lets the hub reach this port.` }
    : route.access === 'PRIVATE'
      ? // R-390: a private route needs only the approval of the person who adds it.
        { title: 'Consent at the machine', label: 'Not needed while the route is private', tone: 'up', detail: `Only your devices reach a private route; ${device} is asked when the route is made public.` }
      : { title: 'Consent at the machine', label: 'Not given', tone: 'unknown', detail: `Nothing can give it yet: the consent at ${device} is not built (OD-HOST-43).` };
  const checks: RouteCheck[] = [{ title: route.proofRecordName === null ? 'The name points at this hub' : 'The name is proven', ...proof }, consent, recordCheck(route)];
  if (!isStream(route.kind)) {
    const certificate = certificateStatus(route, now, locale);
    checks.push({ title: 'Certificate', ...certificate, label: certificate.tone === 'up' ? 'Valid' : certificate.label });
  }
  return checks;
}

/**
 * The records that prove the name. A hosted hub gives the TXT record; the address of the front door is not in the API,
 * so the record that points the name at it has no value. A hub that a person hosts gives no record: the name points at
 * its public address, which the API does not give either.
 */
export function dnsRecords(route: Pick<ApplicationRoute, 'name' | 'proofRecordName' | 'proofRecordValue'>): DnsRecord[] {
  if (route.proofRecordName === null) return [{ type: 'A', name: route.name, valueLabel: 'Address', value: null }];
  return [
    { type: 'TXT', name: route.proofRecordName, valueLabel: 'Value', value: route.proofRecordValue },
    { type: 'CNAME', name: route.name, valueLabel: 'Target', value: null },
  ];
}

/** Why Make public is off. The hub has no step that makes a route public yet, and the consent at the machine comes first. */
export function makePublicRefusal(route: Pick<ApplicationRoute, 'machineConsent' | 'port'>, device: string): string {
  if (!route.machineConsent) {
    return `Consent at ${device} is not given. Port ${String(route.port ?? '')} is open to the hub, not to the internet. An administrator of ${device} turns on "Public" for this port, in Anima on ${device}. That step is not built yet (OD-HOST-43).`;
  }
  return 'The hub has no step that makes a route public yet.';
}

/** What the confirm dialog of Remove says: the reach that ends, that it cannot be undone, and the DNS records left to delete. */
export function removalBody(route: Pick<ApplicationRoute, 'name' | 'access' | 'kind'>, to: { readonly head: string; readonly detail: string }): string {
  const reach = route.access === 'PUBLIC' ? 'the internet' : 'the devices of your Anima network';
  const ends = route.kind === 'REDIRECT' ? `${route.name} stops redirecting to ${to.detail}` : `${route.name} stops reaching ${to.head.replace(/^On /, '')}, ${to.detail}`;
  return `${ends}, for ${reach}. The hub forgets the name, its certificate and its key. You cannot undo this. Delete the DNS records of the name yourself.`;
}
