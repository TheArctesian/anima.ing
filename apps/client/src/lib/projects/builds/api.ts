import { graphql } from '$lib/hub/generated';
import type { ProjectBuildPlacementQuery, SourceBuildInput } from '$lib/hub/generated/graphql';
import type { Hub } from '$lib/hub/hub';

const QUEUE = graphql(`
  mutation QueueProjectSourceBuild($input: SourceBuildInput!) {
    queueSourceBuild(input: $input) { id state attempt reason }
  }
`);

const STATUS = graphql(`
  query ProjectSourceBuildStatus($id: ID!) {
    sourceBuild(id: $id) { id state attempt reason }
  }
`);

const PLACEMENT = graphql(`
  query ProjectBuildPlacement($project: ID!) {
    projectBuildPlacement(project: $project) {
      pinned { id name target privateCheckout }
      candidates { id name target privateCheckout }
    }
  }
`);

const PIN = graphql(`
  mutation SetProjectBuildPin($project: ID!, $device: ID) {
    setProjectBuildPin(project: $project, device: $device) {
      pinned { id name target privateCheckout }
      candidates { id name target privateCheckout }
    }
  }
`);

export type BuildInput = Omit<SourceBuildInput, 'request'> & { request: string };
export interface BuildReceipt { id: string; state: string; attempt: number; reason?: string | null }
export type BuildPlacement = ProjectBuildPlacementQuery['projectBuildPlacement'];

export function sourceBuildApi(hub: Hub) {
  return {
    origin: hub.address.origin,
    queue: (input: BuildInput) => hub.request(QUEUE, { input }),
    status: (id: string) => hub.request(STATUS, { id }),
    placement: (project: string) => hub.request(PLACEMENT, { project }),
    pin: (project: string, device: string | null) => hub.request(PIN, { project, device }),
  };
}

export type SourceBuildApi = ReturnType<typeof sourceBuildApi>;
