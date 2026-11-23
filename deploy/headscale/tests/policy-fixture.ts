import { parseHujson } from '../src/hujson.ts';
import { parsePolicy, type Policy } from '../src/policy.ts';
import { allows, type Connection, type TailnetNode } from '../src/tailnet.ts';

export const policyText = await Bun.file(new URL('../config/policy.hujson', import.meta.url)).text();
export const raw = parseHujson(policyText) as Record<string, unknown>;
export const policy = parsePolicy(raw);

export const SITES = { home: '10.10.0.0/24', office1: '10.20.0.0/24', office2: '10.30.0.0/24' };
export const TAILNET_IPV4 = '100.64.0.0/10';
export const nodes: TailnetNode[] = [
  { name: 'mac', identity: { user: 'operator' }, ip: '100.64.0.2' },
  { name: 'laptop', identity: { user: 'operator' }, ip: '100.64.0.11' },
  { name: 'hub', identity: { tags: ['tag:hub'] }, ip: '100.64.0.1' },
  { name: 'daemon', identity: { tags: ['tag:daemon'] }, ip: '100.64.0.30' },
  { name: 'router-home', identity: { tags: ['tag:router', 'tag:site-home'] }, ip: '100.64.0.20', routes: [SITES.home] },
  { name: 'router-office2', identity: { tags: ['tag:router', 'tag:site-office2'] }, ip: '100.64.0.21', routes: [SITES.office2] },
];

export type From = Connection['from'];
export type To = Connection['to'];
export const tcp = (from: From, to: To, port: number): Connection => ({ from, to, port });
export const allowed = (connection: Connection, under: Policy = policy, inventory = nodes): boolean => allows(under, inventory, connection);
export const withGrants = (grants: unknown[]): Policy => parsePolicy({ ...raw, grants });
export const withTests = (tests: unknown[]): Policy => parsePolicy({ ...raw, tests });

/** Headscale refuses a policy whose tests name a tag no node has, so these rows wait for their node. */
export const ROUTER_ROWS = [
  { src: 'tag:site-home', proto: 'tcp', accept: ['10.10.0.5:22'], deny: ['10.20.0.5:22', '10.30.0.5:22', 'operator@:22', 'tag:hub:443'] },
  { src: '10.10.0.5', deny: ['operator@:22', 'tag:hub:443', '10.30.0.5:22'] },
  { src: 'tag:router', proto: 'tcp', deny: ['operator@:22', 'operator@:443', 'tag:hub:443', 'tag:hub:22'] },
  { src: 'operator@', proto: 'tcp', deny: ['tag:router:22'] },
  { src: 'tag:hub', proto: 'tcp', deny: ['tag:router:22'] },
  { src: 'tag:site-office2', proto: 'tcp', accept: ['10.30.0.5:22'], deny: ['10.10.0.5:22', '10.20.0.5:22', 'operator@:22', 'tag:hub:443'] },
  { src: '10.30.0.5', deny: ['operator@:22', 'tag:hub:443', '10.10.0.5:22'] },
];

export const DAEMON_ROWS = [
  { src: 'tag:daemon', proto: 'tcp', accept: ['tag:hub:443'], deny: ['tag:hub:22', 'tag:hub:80', 'tag:hub:5432', '10.10.0.5:22', '10.20.0.5:443', '10.30.0.5:3389', '100.64.0.2:22', '100.64.0.3:5900', 'operator@:443'] },
  { src: 'tag:daemon', proto: 'udp', deny: ['tag:hub:443', 'tag:hub:53'] },
  { src: 'tag:hub', proto: 'tcp', deny: ['tag:daemon:22', 'tag:daemon:443'] },
  { src: 'operator@', proto: 'tcp', deny: ['tag:daemon:22', 'tag:daemon:443'] },
];

export const secondDaemon: TailnetNode = { name: 'daemon2', identity: { tags: ['tag:daemon'] }, ip: '100.64.0.31' };
export const everyone = [...nodes, secondDaemon];
