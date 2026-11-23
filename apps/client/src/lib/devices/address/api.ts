import { graphql } from '$lib/hub/generated';
import type { DeviceHostsQuery } from '$lib/hub/generated/graphql';
import type { Hub, HubError, HubResult } from '$lib/hub/hub';

const DeviceHosts = graphql(`
  query DeviceHosts($after: String) {
    devices(first: 200, after: $after) {
      pageInfo {
        hasNextPage
        endCursor
      }
      edges {
        node {
          id
          name
          host
          metricsSource
        }
      }
    }
  }
`);

type DeviceHost = DeviceHostsQuery['devices']['edges'][number]['node'];

const MAX_HOST_PAGES = 50;

/** The answer when the hosts cannot all be read: a partial list could miss this computer's device (R-060). */
const HOSTS_INCOMPLETE: HubError = {
  code: 'malformed',
  message: 'The hub did not list every device.',
};

/**
 * Every device's host, page by page, or an error unless the last page was reached: a page that says there is
 * more but gives no cursor, or more than `MAX_HOST_PAGES` pages, fails rather than returning part of the list.
 */
export async function readHosts(hub: Hub): Promise<HubResult<readonly DeviceHost[]>> {
  const hosts: DeviceHost[] = [];
  let after: string | null = null;
  for (let page = 0; page < MAX_HOST_PAGES; page += 1) {
    const result: HubResult<DeviceHostsQuery> = await hub.request(DeviceHosts, { after });
    if (!result.ok) return result;
    const { edges, pageInfo }: DeviceHostsQuery['devices'] = result.value.devices;
    hosts.push(...edges.map((edge) => edge.node));
    if (!pageInfo.hasNextPage) return { ok: true, value: hosts };
    after = pageInfo.endCursor ?? null;
    if (after === null) return { ok: false, error: HOSTS_INCOMPLETE };
  }
  return { ok: false, error: HOSTS_INCOMPLETE };
}
