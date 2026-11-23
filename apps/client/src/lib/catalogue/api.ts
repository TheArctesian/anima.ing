import { graphql } from '$lib/hub/generated';
import type { CatalogueEntriesQuery } from '$lib/hub/generated/graphql';
import type { Hub, HubResult } from '$lib/hub/hub';

const CATALOGUE_ENTRIES = graphql(`
  query CatalogueEntries {
    catalogueEntries {
      id
      name
      description
      kind
      version
      author
      tags
      documentationUrl
      marks
      networkPorts {
        port
        protocols
      }
      class
      tier
      origin {
        publisher
        read
      }
      reviewedBy
      reviewedOn
      streams {
        port
        protocols
      }
      optionalParts {
        kind
        path
        on
        needsMachineSetting
      }
      lastMachineTest
      pinned
      install {
        refusal
        reason
        needsOpenToNetwork
      }
      environmentVariables
      composeFile
    }
  }
`);

export type CatalogueEntry = CatalogueEntriesQuery['catalogueEntries'][number];

export interface CatalogueCalls {
  entries(): Promise<HubResult<readonly CatalogueEntry[]>>;
}

export function catalogueApi(hub: Pick<Hub, 'request'>): CatalogueCalls {
  return {
    entries: async () => {
      const result = await hub.request(CATALOGUE_ENTRIES, {});
      return result.ok ? { ok: true, value: result.value.catalogueEntries } : result;
    },
  };
}
