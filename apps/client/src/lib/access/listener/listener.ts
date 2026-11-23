import type { ListenerResult } from '$lib/desktop/hub-listener/shell';
import type { DeviceLinkStatus } from '$lib/desktop/link/shell';
import type { TokenProvider } from '$lib/hub/hub';

/** F-10: a plain `http` origin is permitted only for this computer; one that does not parse is checked too, and the shell refuses it. */
export function needsListenerCheck(origin: string, desktop: boolean): boolean {
  if (!desktop) return false;
  return !URL.canParse(origin) || new URL(origin).protocol === 'http:';
}

/** The decision for one loopback hub address: send to it only when a program of this user listens (F-10). */
export type ListenerVerdict = { readonly safe: true } | { readonly safe: false; readonly listener: 'other' | 'none' | 'unknown' };

export function verdictOf(result: ListenerResult): ListenerVerdict {
  if (!result.ok) return { safe: false, listener: 'unknown' };
  return result.listener === 'own' ? { safe: true } : { safe: false, listener: result.listener };
}

/** The gate refused: fixed text, so a hub client that reports it says nothing about the address (LOG-007). */
export class ListenerRefused extends Error {
  constructor() {
    super('The hub address on this computer is not served by this app.');
    this.name = 'ListenerRefused';
  }
}

export interface ListenerGate {
  verdict(origin: string): Promise<ListenerVerdict>;
  forget(origin: string): void;
  /** Reads `token` only once the verdict is safe; a `fresh` reader, for the socket that reconnects by itself, asks before every read. */
  token(token: TokenProvider, read?: TokenRead): TokenProvider;
}

export interface TokenRead {
  readonly fresh: boolean;
}

/** A program that takes the port later is seen within this time. */
export const VERDICT_LIFETIME_MS = 5000;

/** What the gate needs: the shell's check and the clock (CQ-021). */
export interface GateParts {
  readonly check: (origin: string) => Promise<ListenerResult>;
  readonly now: () => number;
}

export function listenerGate({ check, now }: GateParts): ListenerGate {
  const verdicts = new Map<string, { readonly asked: Promise<ListenerVerdict>; readonly at: number }>();
  const verdict = (origin: string): Promise<ListenerVerdict> => {
    const known = verdicts.get(origin);
    if (known !== undefined && now() - known.at < VERDICT_LIFETIME_MS) return known.asked;
    const asked = check(origin).then(verdictOf, () => ({ safe: false, listener: 'unknown' }) as const);
    verdicts.set(origin, { asked, at: now() });
    return asked;
  };
  const forget = (origin: string): void => {
    verdicts.delete(origin);
  };
  return {
    verdict,
    forget,
    token:
      (token, read = { fresh: false }) =>
      async (address) => {
        if (read.fresh) forget(address.origin);
        if (!(await verdict(address.origin)).safe) throw new ListenerRefused();
        return token(address);
      },
  };
}

export interface ListenerProblem {
  readonly title: string;
  readonly advice: string;
}

function portOf(origin: string): number | null {
  if (!URL.canParse(origin)) return null;
  const { port } = new URL(origin);
  return port === '' ? null : Number(port);
}

function linkProblem(origin: string, link: DeviceLinkStatus | null): ListenerProblem | null {
  if (link === null || !link.joined || link.port !== portOf(origin)) return null;
  const title = 'Link to the hub is down';
  if (link.reason === 'port-taken') return { title, advice: `Another program holds the link's port ${String(link.port)}. Close it.` };
  if (link.reason === 'not-carried') return { title, advice: 'This build does not carry the link program.' };
  if (link.state === 'starting') return { title: 'Link to the hub is starting', advice: 'Check again in a moment.' };
  if (link.state === 'up') return null;
  return { title, advice: "Check this computer's network." };
}

/** Fixed text; for a joined device the link's state comes first (F-10). */
export function listenerProblem(verdict: ListenerVerdict, origin: string, link: DeviceLinkStatus | null): ListenerProblem | null {
  if (verdict.safe) return null;
  const fromLink = linkProblem(origin, link);
  if (fromLink !== null) return fromLink;
  switch (verdict.listener) {
    case 'other':
      return { title: "Another program holds the hub's port", advice: 'Anima.ing sends it nothing. Stop that program.' };
    case 'none':
      return { title: 'Nothing answers at the hub address', advice: "Start this computer's hub, or the link to your hub." };
    case 'unknown':
      return { title: "Can't check the hub address", advice: 'Nothing is sent to an unchecked address.' };
  }
}
