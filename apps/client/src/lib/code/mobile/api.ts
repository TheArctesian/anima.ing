import { graphql } from '$lib/hub/generated';
import type { MobileThreadInboxQuery, MobileThreadSnapshotQuery } from '$lib/hub/generated/graphql';
import type { Hub, Sink } from '$lib/hub/hub';

graphql(`
  fragment MobileThreadSummary on AgentThreadSummary {
    id
    deviceId
    program
    state
    title
    activityAt
    repository
  }
`);

const Inbox = graphql(`
  query MobileThreadInbox($after: String) {
    agentThreads(after: $after) {
      nodes { ...MobileThreadSummary }
      pageInfo { hasNextPage endCursor }
    }
  }
`);

const Snapshot = graphql(`
  query MobileThreadSnapshot($id: ID!) {
    agentThread(id: $id) {
      summary { ...MobileThreadSummary }
      entries { sequence content }
      lastSequence
    }
  }
`);

const Entries = graphql(`
  subscription MobileThreadEntries($id: ID!, $afterSequence: String!) {
    agentThreadEntries(id: $id, afterSequence: $afterSequence) {
      sequence
      content
    }
  }
`);

export type ThreadPage = MobileThreadInboxQuery['agentThreads'];
export type ThreadSummary = ThreadPage['nodes'][number];
export type ThreadSnapshot = NonNullable<MobileThreadSnapshotQuery['agentThread']>;
export type ThreadEntry = ThreadSnapshot['entries'][number];

export function mobileCodeApi(hub: Hub) {
  return {
    async inbox(after: string | null) {
      const result = await hub.request(Inbox, { after });
      return result.ok ? { ok: true as const, value: result.value.agentThreads } : result;
    },
    async snapshot(id: string) {
      const result = await hub.request(Snapshot, { id });
      return result.ok ? { ok: true as const, value: result.value.agentThread } : result;
    },
    follow(id: string, cursor: () => string, sink: Sink<ThreadEntry>) {
      // The socket serializes its subscription variables again when it retries.
      return hub.subscribe(Entries, { id, get afterSequence() { return cursor(); } }, {
        next: (value) => { sink.next(value.agentThreadEntries); },
        error: sink.error,
        complete: sink.complete,
      });
    },
  };
}

export type MobileCodeApi = ReturnType<typeof mobileCodeApi>;
