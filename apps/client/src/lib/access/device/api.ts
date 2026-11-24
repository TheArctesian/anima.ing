import { graphql } from '$lib/hub/generated';
import { type Hub, type HubResult } from '$lib/hub/hub';

const DETAILS = graphql(`
  mutation CommandLineSignInDetails($userCode: String!) {
    commandLineSignIn(userCode: $userCode) { computer address createdAt expiresAt }
  }
`);
const APPROVE = graphql(`
  mutation ApproveCommandLineSignIn($userCode: String!, $approve: Boolean!) {
    approveCommandLineSignIn(userCode: $userCode, approve: $approve)
  }
`);
const SESSIONS = graphql(`
  query CommandLineSessions($offset: Int!) {
    commandLineSessions(offset: $offset) { id computer createdAt lastUsedAt expiresAt linked current }
  }
`);
const REVOKE = graphql(`
  mutation RevokeCommandLineSession($id: ID!) { revokeCommandLineSession(id: $id) }
`);

export function deviceSignInApi(hub: Hub) {
  return {
    details: async (userCode: string) => mapValue(await hub.request(DETAILS, { userCode }), (value) => value.commandLineSignIn),
    approve: async (userCode: string, approve: boolean) => mapValue(await hub.request(APPROVE, { userCode, approve }), (value) => value.approveCommandLineSignIn),
    sessions: async (offset: number) => mapValue(await hub.request(SESSIONS, { offset }), (value) => value.commandLineSessions),
    revoke: async (id: string) => mapValue(await hub.request(REVOKE, { id }), (value) => value.revokeCommandLineSession),
  };
}

function mapValue<T, U>(result: HubResult<T>, pick: (value: T) => U): HubResult<U> {
  return result.ok ? { ok: true, value: pick(result.value) } : result;
}
