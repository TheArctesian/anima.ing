import type { DocumentNode } from 'graphql';
import type { HubAddress } from '../address';
import { TOKEN_UNREADABLE, type HubResult } from '../error';
import { copiedQuery, copyKey, isShown, type CopyPlace } from './copy';

/** Every part is injected (CQ-021). */
export interface RecentOptions extends CopyPlace {
  readonly address: HubAddress;
  readonly now: () => Date;
  /** Lets the status show the copy's age (NET-004). */
  readonly served: (at: Date) => void;
}

/** A read-only copy of recent answers, given in place of an answer when the hub is unreachable (FND-007). */
export interface RecentCopy {
  answer<TResult>(document: DocumentNode, variables: unknown, send: () => Promise<HubResult<TResult>>): Promise<HubResult<TResult>>;
}

function parsed(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}

async function kept(options: RecentOptions, key: string): Promise<{ at: Date; value: unknown } | undefined> {
  // A store that fails gives no copy: the caller then reports the hub as unreachable, which is the truth.
  const copy = await options.store.read(key).catch(() => undefined);
  if (copy === undefined || !isShown(copy.at, options.now().getTime(), options.limits)) return undefined;
  const value = parsed(copy.text);
  return value === undefined ? undefined : { at: new Date(copy.at), value };
}

function keep(options: RecentOptions, key: string, value: unknown): void {
  const text = JSON.stringify(value);
  const { store, limits } = options;
  const done = text.length > limits.longest ? store.remove(key) : store.write({ key, at: options.now().getTime(), text }, limits.most);
  // The copy is extra: a failing store never fails or slows a good answer.
  done.catch(() => undefined);
}

/** A token that this computer could not read is not a refusal by the hub. */
function refusedByHub(result: HubResult<unknown>): boolean {
  return !result.ok && result.error.code === 'UNAUTHENTICATED' && result.error !== TOKEN_UNREADABLE;
}

export function createRecentCopy(options: RecentOptions): RecentCopy {
  return {
    async answer<TResult>(document: DocumentNode, variables: unknown, send: () => Promise<HubResult<TResult>>) {
      const sentIn = options.era();
      const result = await send();
      if (refusedByHub(result)) {
        // The hub ended this sign-in, so its data does not stay here.
        options.store.clear().catch(() => undefined);
        return result;
      }
      const query = copiedQuery(document);
      if (query === null) return result;
      const key = copyKey(options.address, query, variables);
      if (result.ok) {
        // An answer to a request that was sent before the copy was removed (sign-out) is not kept.
        if (sentIn === options.era()) keep(options, key, result.value);
        return result;
      }
      if (result.error.code !== 'unreachable') return result;
      const copy = await kept(options, key);
      if (copy === undefined) return result;
      options.served(copy.at);
      // The copy is an earlier answer of the hub to the same document, so it has the document's shape (API-008).
      return { ok: true, value: copy.value as TResult };
    },
  };
}
