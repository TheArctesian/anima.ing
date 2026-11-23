// The fixtures hold no live events, so a spec pushes one by hand to the open graphql-ws subscriptions of an operation.

export interface Subscriber {
  readonly operation: string | null;
  deliver(data: Record<string, unknown>): void;
}

export interface Live {
  join(subscriber: Subscriber): () => void;
  push(operation: string, data: Record<string, unknown>): number;
}

export function newLive(): Live {
  const subscribers = new Set<Subscriber>();
  return {
    join(subscriber) {
      subscribers.add(subscriber);
      return () => subscribers.delete(subscriber);
    },
    push(operation, data) {
      const matching = [...subscribers].filter((found) => found.operation === operation);
      for (const subscriber of matching) subscriber.deliver(data);
      return matching.length;
    },
  };
}
