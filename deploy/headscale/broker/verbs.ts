/** Each flag and its value are one argv element, so a value is never read as a flag; no key is reusable or ephemeral (R-055, BD-9, VP-10). */

import { prefixText, type Policy } from '../src/policy.ts';
import { OPERATOR, type BrokerNode } from './nodes.ts';
import { expiryValue, staggered } from './outcome.ts';
import { Refused, type Refusal, type Request } from './protocol.ts';

export interface PolicyView {
  readonly sites: ReadonlyMap<string, string>;
  readonly tags: ReadonlySet<string>;
}

export type Step = readonly string[];

export interface Moment {
  readonly now: Date;
  /** `preauthkeys create --user` takes an id. */
  readonly operatorId: number | null;
}

const JSON_OUTPUT = '--output=json';
const SITE_HOST = /^site-([a-z0-9-]{1,32})$/;
const ROUTER_TAG = 'tag:router';
/** VP-12: `renew` refuses a node whose key expired longer ago than this. */
export const RENEW_GRACE_MS = 30 * 24 * 60 * 60 * 1000;
export const RENEW_TOO_LATE = 'the key expired more than 30 days ago; enrol the computer again';

export const LIST_STEPS: readonly Step[] = [
  ['nodes', 'list', JSON_OUTPUT],
  ['nodes', 'list-routes', JSON_OUTPUT],
];
export const USERS_STEP: Step = ['users', 'list', JSON_OUTPUT];

export function siteTag(site: string): string {
  return `tag:site-${site}`;
}

export function policyView(policy: Policy): PolicyView {
  const sites = new Map<string, string>();
  for (const [host, prefix] of policy.scope.hosts) {
    const site = SITE_HOST.exec(host)?.[1];
    if (site !== undefined) sites.set(site, prefixText(prefix));
  }
  return { sites, tags: policy.scope.tags };
}

function approvedElsewhere(subnet: string, nodes: readonly BrokerNode[], except: number | null): number | undefined {
  return nodes.find((node) => node.id !== except && node.approved.includes(subnet))?.id;
}

function routerKey(site: string, policy: PolicyView, nodes: readonly BrokerNode[]): readonly Step[] | Refusal {
  // BD-9: Headscale accepts a tag the policy does not know, so the broker checks it first.
  const subnet = policy.sites.get(site);
  if (subnet === undefined || !policy.tags.has(siteTag(site)) || !policy.tags.has(ROUTER_TAG)) return 'unknown-site';
  // M2: a site has one router.
  if (approvedElsewhere(subnet, nodes, null) !== undefined) return 'not-allowed';
  return [['preauthkeys', 'create', `--tags=${ROUTER_TAG},${siteTag(site)}`, '--expiration=1h', JSON_OUTPUT]];
}

function approveRoute(request: { node: number; site: string }, policy: PolicyView, nodes: readonly BrokerNode[]): readonly Step[] | Refusal {
  const subnet = policy.sites.get(request.site);
  if (subnet === undefined) return 'unknown-site';
  const node = nodes.find((candidate) => candidate.id === request.node);
  if (node === undefined || !node.tags.includes(ROUTER_TAG) || !node.tags.includes(siteTag(request.site))) return 'not-allowed';
  if (node.available.length !== 1 || node.available[0] !== subnet) return 'not-allowed';
  if (approvedElsewhere(subnet, nodes, node.id) !== undefined) return 'not-allowed';
  return [['nodes', 'approve-routes', `--identifier=${String(node.id)}`, `--routes=${subnet}`]];
}

export function expiredTooLong(node: BrokerNode, now: Date): boolean {
  return node.expiry !== null && now.getTime() - Date.parse(node.expiry) > RENEW_GRACE_MS;
}

function renew(nodeId: number, nodes: readonly BrokerNode[], now: Date): readonly Step[] | Refusal {
  const node = nodes.find((candidate) => candidate.id === nodeId);
  if (node === undefined || node.tags.length > 0 || node.user !== OPERATOR) return 'not-allowed';
  if (expiredTooLong(node, now)) throw new Refused('not-allowed', RENEW_TOO_LATE);
  const others = nodes
    .filter((other) => other.id !== nodeId)
    .map((other) => ({ ...other, expiry: other.expiry === null ? null : new Date(other.expiry) }));
  return [['nodes', 'expire', `--identifier=${String(nodeId)}`, `--expiry=${expiryValue(staggered(now, others))}`]];
}

export function plan(request: Request, policy: PolicyView, nodes: readonly BrokerNode[], moment: Moment): readonly Step[] | Refusal {
  switch (request.verb) {
    case 'nodes':
      return LIST_STEPS;
    case 'personal-key':
      if (moment.operatorId === null) return 'headscale-failed';
      return [['preauthkeys', 'create', `--user=${String(moment.operatorId)}`, '--expiration=1h', JSON_OUTPUT]];
    case 'router-key':
      return routerKey(request.site, policy, nodes);
    case 'approve-route':
      return approveRoute(request, policy, nodes);
    case 'register':
      // `auth register --user` takes the user's name (unlike `preauthkeys create`; Build notes).
      return [['auth', 'register', `--user=${OPERATOR}`, `--auth-id=${request.authId}`, JSON_OUTPUT]];
    case 'renew':
      return renew(request.node, nodes, moment.now);
  }
}
