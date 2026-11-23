import { graphql } from '$lib/hub/generated';
import type { OpenFilesMutation } from '$lib/hub/generated/graphql';
import type { Hub, HubResult } from '$lib/hub/hub';

const OpenFiles = graphql(`
  mutation OpenFiles($sessionId: ID!) {
    openFiles(sessionId: $sessionId) {
      sessionId
      token
      tunnelPath
      expiresAt
    }
  }
`);

/** A single-use files-tunnel ticket (K1 `SessionTicket`, K4 §9, API-009). */
export type FilesTicket = OpenFilesMutation['openFiles'];

/** Calls go through a `Hub`; failures stay its typed K7 errors (CQ-045). */
export interface FilesApi {
  /** The token is single-use and is never kept or shown (SEC-002, LOG-007). */
  openFiles(sessionId: string): Promise<HubResult<FilesTicket>>;
}

export function filesApi(hub: Hub): FilesApi {
  return {
    openFiles: async (sessionId) => {
      const result = await hub.request(OpenFiles, { sessionId });
      return result.ok ? { ok: true, value: result.value.openFiles } : result;
    },
  };
}
