import { graphql } from '$lib/hub/generated';
import type { EventsQuery, EventsQueryVariables, Severity } from '$lib/hub/generated/graphql';
import type { Hub, HubResult, Sink } from '$lib/hub/hub';

const Events = graphql(`
  query Events($filter: EventFilter, $first: Int, $after: String) {
    events(filter: $filter, first: $first, after: $after) {
      pageInfo {
        hasNextPage
        endCursor
      }
      edges {
        node {
          id
          at
          severity
          source
          deviceId
          deviceName
          message
        }
      }
    }
  }
`);

const EventAdded = graphql(`
  subscription EventAdded($minSeverity: Severity) {
    eventAdded(minSeverity: $minSeverity) {
      id
      at
      severity
      source
      deviceId
      deviceName
      message
    }
  }
`);

export type EventPage = EventsQuery['events'];

export type LogEvent = EventPage['edges'][number]['node'];

/** CQ-045. */
export function eventsApi(hub: Hub) {
  return {
    list: async (variables: EventsQueryVariables): Promise<HubResult<EventPage>> => {
      const result = await hub.request(Events, variables);
      return result.ok ? { ok: true, value: result.value.events } : result;
    },
    /** Events of `minSeverity` and above (LOG-010). */
    watch: (minSeverity: Severity, sink: Sink<LogEvent>): (() => void) =>
      hub.subscribe(
        EventAdded,
        { minSeverity },
        {
          next: (value) => {
            sink.next(value.eventAdded);
          },
          error: (error) => {
            sink.error(error);
          },
          complete: () => {
            sink.complete();
          },
        },
      ),
  };
}
