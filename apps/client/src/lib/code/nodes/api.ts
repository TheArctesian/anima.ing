import { graphql } from '$lib/hub/generated';
import type { Workspace } from './receipts';
import type { Hub, Sink } from '$lib/hub/hub';
import type { Action as WorkspaceAction } from './workspace/records';

const Workspaces = graphql(`
  query CodeNodeWorkspaces($device: ID!) { managedCodeWorkspaces(device: $device) }
`);
const Enable = graphql(`
  mutation CodeNodeEnable($device: ID!, $workspace: ID!, $enabled: Boolean!) {
    setManagedCodeWorkspaceEnabled(device: $device, input: { workspace: $workspace, enabled: $enabled })
  }
`);
const Submit = graphql(`
  mutation CodeNodeSubmit($device: ID!, $input: ManagedCodeInput!) {
    queueManagedCodeCommand(device: $device, input: $input)
  }
`);
const Receipt = graphql(`
  query CodeNodeReceipt($request: ID!) { managedCodeCommand(request: $request) }
`);
const Previews = graphql(`
  query CodeNodePreviews($device: ID!, $workspace: ID!) { managedCodePreviews(device: $device, workspace: $workspace) }
`);
const OpenPreview = graphql(`
  mutation CodeNodeOpenPreview($device: ID!, $input: ManagedCodePreviewInput!) { openManagedCodePreview(device: $device, input: $input) }
`);
const ClosePreview = graphql(`
  mutation CodeNodeClosePreview($endpoint: ID!) { closeManagedCodePreview(endpoint: $endpoint) }
`);
const PreviewHandoff = graphql(`
  mutation CodeNodePreviewHandoff($endpoint: ID!) { managedCodePreviewHandoff(endpoint: $endpoint) }
`);
const WorkspaceInventory = graphql(`query CodeWorkspaceInventory($device: ID!) { managedNodeWorkspaces(device: $device) }`);
const WorkspaceReceipt = graphql(`query CodeWorkspaceReceipt($request: ID!) { managedWorkspaceRequest(request: $request) }`);
const WorkspaceSubmit = graphql(`mutation CodeWorkspaceSubmit($device: ID!, $input: ManagedWorkspaceInput!) { queueManagedWorkspaceRequest(device: $device, input: $input) }`);
const WorkspaceRepositories = graphql(`
  query CodeWorkspaceRepositories($after: String) {
    projects(first: 100, after: $after) {
      edges { node { repositories { id fullName private archived leftOut gone access } } }
      pageInfo { hasNextPage endCursor }
    }
  }
`);
const ProviderLogin = graphql(`
  subscription CodeNodeProviderLogin($device: ID!, $workspace: ID!, $generation: String!, $provider: String) {
    managedCodeProviderLogin(device: $device, input: { workspace: $workspace, generation: $generation, provider: $provider })
  }
`);
const ProviderReply = graphql(`mutation CodeNodeProviderReply($session: ID!, $code: String!) { replyManagedCodeProviderLogin(session: $session, code: $code) }`);
export function nodeCodeApi(hub: Hub) {
  return {
    login: (device: string, workspace: Pick<Workspace, 'workspace' | 'generation'>, provider: string, sink: Sink<unknown>) =>
      hub.subscribe(ProviderLogin, { device, workspace: workspace.workspace, generation: String(workspace.generation), provider }, {
        next: (reply) => sink.next(reply.managedCodeProviderLogin),
        error: sink.error,
        complete: sink.complete,
      }, { retry: false }),
    workspaceInventory: (device: string) => hub.request(WorkspaceInventory, { device }),
    workspaceReceipt: (request: string) => hub.request(WorkspaceReceipt, { request }),
    workspaceRepositories: (after: string | null) => hub.request(WorkspaceRepositories, { after }),
    submitWorkspace: (device: string, request: string, workspace: string, action: WorkspaceAction) => hub.request(WorkspaceSubmit, { device, input: { request, workspace, action } }),
    withdrawWorkspace: (device: string, request: string, workspace: string) => hub.request(WorkspaceSubmit, {
      device, input: { request, workspace, action: { kind: 'withdraw-move' } },
    }),
    retireWorkspaceSource: (device: string, request: string, workspace: string) => hub.request(WorkspaceSubmit, {
      device, input: { request, workspace, action: { kind: 'retire-source', confirmed: true } },
    }),
    previews: (device: string, workspace: string) => hub.request(Previews, { device, workspace }),
    openPreview: (device: string, endpoint: string, workspace: Pick<Workspace, 'workspace' | 'generation'>, port: number) => hub.request(OpenPreview, { device, input: { endpoint, workspace: workspace.workspace, generation: String(workspace.generation), port } }),
    closePreview: (endpoint: string) => hub.request(ClosePreview, { endpoint }),
    previewHandoff: (endpoint: string) => hub.request(PreviewHandoff, { endpoint }),
    workspaces: (device: string) => hub.request(Workspaces, { device }),
    replyLogin: (session: string, code: string) => hub.request(ProviderReply, { session, code }),
    enable: (device: string, workspace: string, enabled: boolean) => hub.request(Enable, { device, workspace, enabled }),
    submit: (device: string, request: string, workspace: Pick<Workspace, 'workspace' | 'generation'>, action: unknown) =>
      hub.request(Submit, { device, input: { request, workspace: workspace.workspace, generation: String(workspace.generation), action } }),
    receipt: (request: string) => hub.request(Receipt, { request }),
  };
}
export type NodeCodeApi = ReturnType<typeof nodeCodeApi>;
