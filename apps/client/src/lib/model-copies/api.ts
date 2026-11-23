import { graphql } from '$lib/hub/generated';
import type { HostingFoundModelsQuery, HostingModelCopiesQuery, ModelPullChoicesQuery } from '$lib/hub/generated/graphql';
import type { Hub, HubResult } from '$lib/hub/hub';

// The fragment is only read by codegen, which inlines it into the documents that spread it.
void graphql(`
  fragment CopyFields on ModelCopy {
    id
    place {
      kind
      id
      name
    }
    address
    origin
    format
    quantisation
    repository
    sizeBytes
    fileCount
    state
    progress {
      bytes
      totalBytes
      bytesPerSecond
      secondsLeft
    }
    reason
    createdAt
  }
`);

const COPIES = graphql(`
  query HostingModelCopies {
    modelCopies {
      ...CopyFields
    }
  }
`);

// The machines with their device policy, and the variants of the library: what a pull can name.
const CHOICES = graphql(`
  query ModelPullChoices {
    devicePolicies {
      device {
        id
        name
      }
      takesModelWork
      stopReason
    }
    libraryModels {
      variants {
        address
        sizeBytes
      }
    }
  }
`);

const PULL = graphql(`
  mutation HostingPullModel($deviceId: ID!, $model: String!) {
    pullModel(deviceId: $deviceId, model: $model) {
      ...CopyFields
    }
  }
`);

const REMOVE = graphql(`
  mutation HostingRemoveModelCopy($id: ID!) {
    removeModelCopy(id: $id) {
      ...CopyFields
    }
  }
`);

// The models that the machines found where Ollama, LM Studio, the Hugging Face cache or a folder keeps them (PULL-012).
const FOUND = graphql(`
  query HostingFoundModels {
    foundModels {
      found
      place {
        kind
        id
        name
      }
      tool
      name
      format
      fileCount
      sizeBytes
      usable
      readAt
    }
  }
`);

const FIND = graphql(`
  mutation HostingFindModels($deviceId: ID!) {
    findModels(deviceId: $deviceId)
  }
`);

const USE = graphql(`
  mutation HostingUseFoundModel($deviceId: ID!, $found: String!) {
    useFoundModel(deviceId: $deviceId, found: $found) {
      ...CopyFields
    }
  }
`);

export type ModelCopy = HostingModelCopiesQuery['modelCopies'][number];
export type CopyState = ModelCopy['state'];
export type FoundModel = HostingFoundModelsQuery['foundModels'][number];
type Policy = ModelPullChoicesQuery['devicePolicies'][number];

/** A machine that a pull can name, with why it takes no model work now (null when it takes it). */
export interface PullMachine {
  readonly id: string;
  readonly name: string;
  readonly stop: Policy['stopReason'];
}

/** A variant of the library, name:tag, with the size of its files. */
export interface PullVariant {
  readonly address: string;
  readonly sizeBytes: number;
}

export interface PullChoices {
  readonly machines: readonly PullMachine[];
  readonly variants: readonly PullVariant[];
}

/** What the section Models asks of the hub (HSE-013, PULL-001, PULL-007, PULL-009, PULL-012); its store and tests take this. */
export interface CopyCalls {
  /** Each copy that a machine has, pulls or removes. */
  list(): Promise<HubResult<readonly ModelCopy[]>>;
  /** The machines and the variants, by name. */
  choices(): Promise<HubResult<PullChoices>>;
  /** Asks one machine to download one variant name:tag and verify it. */
  pull(deviceId: string, address: string): Promise<HubResult<ModelCopy>>;
  /** Removes a copy: what is left of it (state REMOVING), or null when it went at once. */
  remove(id: string): Promise<HubResult<ModelCopy | null>>;
  /** The models that the machines found, from the last model report of each, with no copy that uses them (PULL-012). */
  found(): Promise<HubResult<readonly FoundModel[]>>;
  /** Asks one machine to read its found models again: the job. */
  find(deviceId: string): Promise<HubResult<string>>;
  /** Uses a found model where it is (R-444): the new copy. */
  use(deviceId: string, found: string): Promise<HubResult<ModelCopy>>;
}

/** The copy calls of the hub. */
export function copiesApi(hub: Pick<Hub, 'request'>): CopyCalls {
  return {
    list: async () => {
      const result = await hub.request(COPIES, {});
      return result.ok ? { ok: true, value: result.value.modelCopies } : result;
    },
    choices: async () => {
      const result = await hub.request(CHOICES, {});
      if (!result.ok) return result;
      const machines = result.value.devicePolicies
        .map((policy) => ({ id: policy.device.id, name: policy.device.name, stop: policy.takesModelWork ? null : policy.stopReason }))
        .sort((a, b) => a.name.localeCompare(b.name));
      const variants = result.value.libraryModels.flatMap((model) => model.variants).sort((a, b) => a.address.localeCompare(b.address));
      return { ok: true, value: { machines, variants } };
    },
    pull: async (deviceId, address) => {
      const result = await hub.request(PULL, { deviceId, model: address });
      return result.ok ? { ok: true, value: result.value.pullModel } : result;
    },
    remove: async (id) => {
      const result = await hub.request(REMOVE, { id });
      return result.ok ? { ok: true, value: result.value.removeModelCopy ?? null } : result;
    },
    found: async () => {
      const result = await hub.request(FOUND, {});
      return result.ok ? { ok: true, value: result.value.foundModels } : result;
    },
    find: async (deviceId) => {
      const result = await hub.request(FIND, { deviceId });
      return result.ok ? { ok: true, value: result.value.findModels } : result;
    },
    use: async (deviceId, found) => {
      const result = await hub.request(USE, { deviceId, found });
      return result.ok ? { ok: true, value: result.value.useFoundModel } : result;
    },
  };
}
