/** The broker's view of Headscale (BD-9); no key field passes, but a new pre-auth key goes to the caller once. */

import { InventoryError, parseNode, parseRoutes, type ListedNode, type Tailnet } from '../src/inventory.ts';
import type { PolicyView } from './verbs.ts';

export interface BrokerNode {
  readonly id: number;
  readonly name: string;
  readonly user: string;
  readonly tags: readonly string[];
  readonly ips: readonly string[];
  readonly online: boolean;
  readonly expiry: string | null;
  readonly approved: readonly string[];
  readonly available: readonly string[];
}

export interface NodesReply {
  readonly nodes: readonly BrokerNode[];
  readonly sites: readonly { readonly name: string; readonly subnet: string }[];
}

export interface BrokerTailnet extends Tailnet {
  /** Unreadable nodes count against `max_nodes` and are never audited or returned (L3). */
  readonly unreadable: readonly (number | null)[];
}

export interface NewKey {
  readonly id: number;
  readonly key: string;
  readonly expiresAt: Date;
}

export interface Registered {
  readonly name: string;
  readonly address: string;
  readonly keyExpiry: string | null;
}

export const OPERATOR = 'operator';

type Json = Readonly<Record<string, unknown>>;

function object(value: unknown, what: string): Json {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) throw new InventoryError(`${what} is not an object`);
  return value as Json;
}

// Headscale writes JSON numbers (vpn-build.md Section 3, fact 14); a protobuf writer would use strings.
function id(value: unknown, what: string): number {
  const number = typeof value === 'string' && /^\d+$/.test(value) ? Number(value) : value;
  if (typeof number !== 'number' || !Number.isSafeInteger(number) || number <= 0) throw new InventoryError(`${what} is not an id`);
  return number;
}

function idOrNull(entry: unknown): number | null {
  try {
    return id(object(entry, 'node').id, 'node id');
  } catch {
    return null;
  }
}

/** Headscale 0.29.7 prints `null` for an empty list; a node in an unknown shape is set aside, not fatal (L3). */
export function tailnetOf(nodes: unknown, routes: unknown): BrokerTailnet {
  const entries = nodes ?? [];
  if (!Array.isArray(entries)) throw new InventoryError('nodes is not a list');
  const read: ListedNode[] = [];
  const unreadable: (number | null)[] = [];
  for (const entry of entries as readonly unknown[]) {
    try {
      read.push(parseNode(entry));
    } catch (error) {
      if (!(error instanceof InventoryError)) throw error;
      unreadable.push(idOrNull(entry));
    }
  }
  return { nodes: read, routes: parseRoutes(routes ?? []), unreadable };
}

export function nodeCountOf(tailnet: BrokerTailnet): number {
  return tailnet.nodes.length + tailnet.unreadable.length;
}

export function nodeIdsOf(tailnet: BrokerTailnet): readonly number[] {
  return [...tailnet.nodes.map((node) => node.id), ...tailnet.unreadable.filter((entry): entry is number => entry !== null)];
}

function brokerNode(node: ListedNode, tailnet: Tailnet): BrokerNode {
  const routes = tailnet.routes.find((entry) => entry.id === node.id);
  return {
    id: node.id,
    name: node.name,
    user: node.user,
    tags: node.tags,
    ips: node.ips,
    online: node.online,
    expiry: node.expiry === null ? null : node.expiry.toISOString(),
    approved: routes?.approved ?? [],
    available: routes?.available ?? [],
  };
}

export function nodesReply(tailnet: Tailnet, policy: PolicyView): NodesReply {
  return {
    nodes: tailnet.nodes.map((node) => brokerNode(node, tailnet)),
    sites: [...policy.sites].map(([name, subnet]) => ({ name, subnet })),
  };
}

export function operatorIdOf(users: unknown): number | null {
  if (users === null) return null;
  if (!Array.isArray(users)) throw new InventoryError('users is not a list');
  const operator = (users as readonly unknown[]).map((entry) => object(entry, 'user')).find((user) => user.name === OPERATOR);
  return operator === undefined ? null : id(operator.id, 'user id');
}

/** Reads `preauthkeys create -o json`: `{ id, key, expiration: { seconds, nanos }, … }`. */
export function newKeyOf(reply: unknown): NewKey {
  const body = object(reply, 'pre-auth key reply');
  if (typeof body.key !== 'string' || body.key === '') throw new InventoryError('the pre-auth key reply has no key');
  const seconds = object(body.expiration, 'expiration').seconds;
  if (typeof seconds !== 'number' || !Number.isSafeInteger(seconds)) throw new InventoryError('expiration.seconds is not a number');
  return { id: id(body.id, 'pre-auth key id'), key: body.key, expiresAt: new Date(seconds * 1000) };
}

/** `auth register -o json` returns the node itself. */
export function registeredIdOf(reply: unknown): number {
  return id(object(reply, 'register reply').id, 'node id');
}

export function registered(node: ListedNode): Registered {
  const address = node.ips.find((ip) => !ip.includes(':'));
  if (address === undefined) throw new InventoryError('the node has no IPv4 address');
  return { name: node.name, address, keyExpiry: node.expiry === null ? null : node.expiry.toISOString() };
}
