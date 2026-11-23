/** The `register` verb (VPN-003); `pendingRegister` is saved first, so a cut-off step still gets its audit (H1). */

import type { ListedNode } from '../src/inventory.ts';
import { OPERATOR, registered, registeredIdOf, type BrokerTailnet, type Registered } from './nodes.ts';
import { DETAILS, Refused } from './protocol.ts';
import { settle, type Run } from './settle.ts';
import { writeState, type BrokerState } from './state.ts';
import type { Step } from './verbs.ts';

export function nodeReply(tailnet: BrokerTailnet, id: number): Registered {
  const node = tailnet.nodes.find((candidate) => candidate.id === id);
  if (node === undefined) throw new Refused('headscale-failed', 'the node is not in the node list');
  return registered(node);
}

function foreign(id: number, before: BrokerTailnet): boolean {
  if (before.unreadable.includes(id)) return true;
  const node: ListedNode | undefined = before.nodes.find((candidate) => candidate.id === id);
  return node !== undefined && (node.tags.length > 0 || node.user !== OPERATOR);
}

async function step(run: Run, state: BrokerState, register: Step): Promise<number> {
  try {
    return registeredIdOf(JSON.parse(await run.runner(register)));
  } catch (error) {
    // The step may have registered a node before it was cut off.
    await settle(run, state);
    throw error;
  }
}

/** The node id comes from the reply, not from a difference of two lists: another node may join. */
export async function register(run: Run, state: BrokerState, registerStep: Step, before: BrokerTailnet): Promise<Registered> {
  const id = await step(run, state, registerStep);
  run.trace.node = id;
  if (foreign(id, before)) {
    // Deleting it would let an auth ID remove a router (L2).
    await settle(run, state);
    throw new Refused('not-allowed', 'the auth ID belongs to a tagged node or to another user; the node was left as it was');
  }
  const pending: BrokerState = { ...state, pendingRegister: { before: state.pendingRegister?.before ?? [], node: id } };
  await writeState(run.files.state, pending);
  const settled = await settle(run, pending);
  const undone = settled.fixes.find((fix) => fix.node === id && fix.kind === 'delete');
  if (undone?.kind === 'delete') throw new Refused('undone', `${DETAILS.undone}: ${undone.reason}`);
  return nodeReply(settled.tailnet, id);
}
