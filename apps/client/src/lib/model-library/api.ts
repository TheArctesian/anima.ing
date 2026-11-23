import { graphql } from '$lib/hub/generated';
import type { HostingLibraryQuery, HostingModelFitsQuery } from '$lib/hub/generated/graphql';
import type { Hub, HubResult } from '$lib/hub/hub';
import { copiesApi } from '$lib/model-copies/api';

// The models of the library with what a person compares them by (SEL-005, LIBM-008).
const LIBRARY = graphql(`
  query HostingLibrary {
    libraryModels {
      name
      family
      parameterCount
      contextLength
      capabilities
      licence
      publisher
      aliases
      variants {
        address
        tag
        format
        quantisation
        sizeBytes
      }
    }
  }
`);

// The fit of each variant of one model on each machine (FIT-001 to FIT-005, task L1-20).
const FITS = graphql(`
  query HostingModelFits($model: String!, $filter: ModelFitFilter) {
    modelFits(model: $model, filter: $filter) {
      variant
      address
      place {
        kind
        id
        name
      }
      state
      placement
      backend
      contextTokens
      largestContext
      sizeBytes
      neededBytes
      usableBytes
      reason
    }
  }
`);

export type LibraryModel = HostingLibraryQuery['libraryModels'][number];
export type LibraryVariant = LibraryModel['variants'][number];
export type ModelFit = HostingModelFitsQuery['modelFits'][number];

/** What the Library of Hosting asks of the hub; its store and tests take this. */
export interface LibraryCalls {
  /** The models of the library, in name order. */
  models(): Promise<HubResult<readonly LibraryModel[]>>;
  /** The fit of each variant of `model` on each machine, at `context` tokens or the default. */
  fits(model: string, context: number | null): Promise<HubResult<readonly ModelFit[]>>;
  /** Asks one machine to download one variant name:tag (PULL-001). */
  pull(deviceId: string, address: string): Promise<HubResult<unknown>>;
}

/** The library calls of the hub. */
export function libraryApi(hub: Pick<Hub, 'request'>): LibraryCalls {
  const copies = copiesApi(hub);
  return {
    models: async () => {
      const result = await hub.request(LIBRARY, {});
      return result.ok ? { ok: true, value: result.value.libraryModels } : result;
    },
    fits: async (model, context) => {
      const result = await hub.request(FITS, { model, filter: { contextTokens: context } });
      return result.ok ? { ok: true, value: result.value.modelFits } : result;
    },
    pull: async (deviceId, address) => copies.pull(deviceId, address),
  };
}
