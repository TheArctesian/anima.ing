import { graphql } from '$lib/hub/generated';
import type { ApiKeyInput, ApiKeysQuery } from '$lib/hub/generated/graphql';
import type { Hub, HubResult } from '$lib/hub/hub';

// Own documents: the K1 fields are @pending, so a hub without them fails only the API keys section.
const API_KEYS = graphql(`
  query ApiKeys {
    apiKeys {
      id
      name
      scope
      madeFor
      prefix
      expiresAt
      createdAt
      lastUsedAt
    }
  }
`);

const CREATE_API_KEY = graphql(`
  mutation CreateApiKey($input: ApiKeyInput!) {
    createApiKey(input: $input) {
      key {
        id
        name
        scope
        madeFor
        prefix
        expiresAt
        createdAt
        lastUsedAt
      }
      secret
    }
  }
`);

const REVOKE_API_KEY = graphql(`
  mutation RevokeApiKey($id: ID!) {
    revokeApiKey(id: $id)
  }
`);

/** One API key as the hub lists it; the secret is never part of it (INT-003). */
export type ApiKey = ApiKeysQuery['apiKeys'][number];

export type { ApiKeyInput };

/** `secret` is the whole key, shown once (INT-003); it is never logged, stored or sent (LOG-007). */
interface CreatedApiKey {
  readonly key: ApiKey;
  readonly secret: string;
}

export interface ApiKeysApi {
  list(): Promise<HubResult<readonly ApiKey[]>>;
  create(input: ApiKeyInput): Promise<HubResult<CreatedApiKey>>;
  /** The id of the revoked key; the key stops working at once. */
  revoke(id: string): Promise<HubResult<string>>;
}

function mapValue<T, U>(result: HubResult<T>, pick: (value: T) => U): HubResult<U> {
  return result.ok ? { ok: true, value: pick(result.value) } : result;
}

export function apiKeysApi(hub: Hub): ApiKeysApi {
  return {
    list: async () => mapValue(await hub.request(API_KEYS, {}), (value) => value.apiKeys),
    create: async (input) => mapValue(await hub.request(CREATE_API_KEY, { input }), (value) => value.createApiKey),
    revoke: async (id) => mapValue(await hub.request(REVOKE_API_KEY, { id }), (value) => value.revokeApiKey),
  };
}
