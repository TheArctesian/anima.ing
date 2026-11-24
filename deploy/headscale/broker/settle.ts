/** The broker's audit run (VPN-003, H2); the audit timer runs it alone, so a bad node is undone while the hub is silent. */

import { tailnetOf, type BrokerTailnet } from './nodes.ts';
import { audit, fixStep, type Fix } from './outcome.ts';
import { Refused } from './protocol.ts';
import { readState, writeState, type BrokerFiles, type BrokerState } from './state.ts';
import { LIST_STEPS, type Step } from './verbs.ts';

export type Runner = (step: Step) => Promise<string>;

export class StepError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'StepError';
  }
}

/** The log line holds no key, auth ID or node name (LOG-007). */
export interface Trace {
  node: number | null;
  fixes: number;
  unreadable: number;
}

export interface Run {
  readonly runner: Runner;
  readonly files: BrokerFiles;
  readonly now: Date;
  readonly trace: Trace;
}

export interface Settled {
  readonly state: BrokerState;
  readonly tailnet: BrokerTailnet;
  readonly fixes: readonly Fix[];
}

export const newTrace = (): Trace => ({ node: null, fixes: 0, unreadable: 0 });

export function logLine(verb: string, outcome: string, trace: Trace): string {
  const unreadable = trace.unreadable > 0 ? ` unreadable=${String(trace.unreadable)}` : '';
  return `anima-broker verb=${verb} outcome=${outcome} node=${trace.node === null ? '-' : String(trace.node)} fixes=${String(trace.fixes)}${unreadable}`;
}

export async function json(run: Run, step: Step): Promise<unknown> {
  return JSON.parse(await run.runner(step)) as unknown;
}

export async function readTailnet(run: Run): Promise<BrokerTailnet> {
  const [nodes, routes] = LIST_STEPS;
  if (nodes === undefined || routes === undefined) throw new Refused('headscale-failed');
  const tailnet = tailnetOf(await json(run, nodes), await json(run, routes));
  run.trace.unreadable = tailnet.unreadable.length;
  return tailnet;
}

/** A failed fix throws before the save, so the next run audits the same nodes again. */
export async function settle(run: Run, state: BrokerState): Promise<Settled> {
  const before = await readTailnet(run);
  const result = audit(before, state.issued, state.pendingRegister, run.now);
  for (const fix of result.fixes) {
    await run.runner(fixStep(fix));
    run.trace.fixes += 1;
  }
  const next = { ...state, issued: result.issued, pendingRegister: null };
  await writeState(run.files.state, next);
  return { state: next, tailnet: result.fixes.length === 0 ? before : await readTailnet(run), fixes: result.fixes };
}

export async function auditTailnet(runner: Runner, files: BrokerFiles, now: Date): Promise<string> {
  const run: Run = { runner, files, now, trace: newTrace() };
  await settle(run, await readState(files.state));
  return logLine('audit', 'ok', run.trace);
}
