import { graphql } from '$lib/hub/generated';
import type {
  AgentThreadAnswerInput, AgentThreadPromptInput, AgentThreadStartInput, AgentThreadStopInput,
  MobileRemoteControlQuery, MobileRemoteTargetsQuery, MobileLaunchReceiptFragment, MobileCommandReceiptFragment,
  MobileThreadInboxQuery, MobileThreadSnapshotQuery,
} from '$lib/hub/generated/graphql';
import type { Hub, HubResult, Sink } from '$lib/hub/hub';

graphql(`
  fragment MobileThreadSummary on AgentThreadSummary {
    id
    deviceId
    program
    state
    title
    activityAt
    repository
  }
`);

const Inbox = graphql(`
  query MobileThreadInbox($after: String) {
    agentThreads(after: $after) {
      nodes { ...MobileThreadSummary }
      pageInfo { hasNextPage endCursor }
    }
  }
`);

const Snapshot = graphql(`
  query MobileThreadSnapshot($id: ID!) {
    agentThread(id: $id) {
      summary { ...MobileThreadSummary }
      entries { sequence content }
      lastSequence
    }
  }
`);

const Summary = graphql(`
  query MobileThreadSummaryRefresh($id: ID!) {
    agentThread(id: $id) {
      summary { ...MobileThreadSummary }
      lastSequence
    }
  }
`);

const Entries = graphql(`
  subscription MobileThreadEntries($id: ID!, $afterSequence: String!) {
    agentThreadEntries(id: $id, afterSequence: $afterSequence) {
      sequence
      content
    }
  }
`);

export type ThreadPage = MobileThreadInboxQuery['agentThreads'];
export type ThreadSummary = ThreadPage['nodes'][number];
export type ThreadSnapshot = NonNullable<MobileThreadSnapshotQuery['agentThread']>;
export type ThreadEntry = ThreadSnapshot['entries'][number];

export function mobileCodeApi(hub: Hub) {
  return {
    async inbox(after: string | null) {
      const result = await hub.request(Inbox, { after });
      return result.ok ? { ok: true as const, value: result.value.agentThreads } : result;
    },
    async snapshot(id: string) {
      const result = await hub.request(Snapshot, { id });
      return result.ok ? { ok: true as const, value: result.value.agentThread } : result;
    },
    async summary(id: string) {
      const result = await hub.request(Summary, { id });
      return result.ok ? { ok: true as const, value: result.value.agentThread } : result;
    },
    follow(id: string, cursor: () => string, sink: Sink<ThreadEntry>) {
      // The socket serializes its subscription variables again when it retries.
      return hub.subscribe(Entries, { id, get afterSequence() { return cursor(); } }, {
        next: (value) => { sink.next(value.agentThreadEntries); },
        error: sink.error,
        complete: sink.complete,
      });
    },
  };
}

export type MobileCodeApi = ReturnType<typeof mobileCodeApi>;

graphql(`
  fragment MobileLaunchReceipt on AgentThreadLaunchReceipt {
    launch state cause thread binding firstCommand
  }
`);

graphql(`
  fragment MobileCommandReceipt on AgentThreadCommandReceipt {
    command state cause
  }
`);

const Targets = graphql(`
  query MobileRemoteTargets {
    agentThreadStartTargets { device project folder program expiresAt }
  }
`);

const Launch = graphql(`
  query MobileRemoteLaunch($launch: ID!) {
    agentThreadLaunch(launch: $launch) { ...MobileLaunchReceipt }
  }
`);

const Command = graphql(`
  query MobileRemoteCommand($command: ID!) {
    agentThreadCommand(command: $command) { ...MobileCommandReceipt }
  }
`);

const Control = graphql(`
  query MobileRemoteControl($thread: ID!) {
    agentThreadControl(thread: $thread) {
      thread
      offer {
        process revision actions
        stopTargets { kind process turn }
        requests {
          kind id subject details decisions rules { selector label }
          questions { key text choices { id label } many freeText }
        }
      }
    }
  }
`);

const Start = graphql(`
  mutation MobileRemoteStart($input: AgentThreadStartInput!) {
    startAgentThread(input: $input) { ...MobileLaunchReceipt }
  }
`);

const Prompt = graphql(`
  mutation MobileRemotePrompt($input: AgentThreadPromptInput!) {
    sendAgentThreadPrompt(input: $input) { ...MobileCommandReceipt }
  }
`);

const Answer = graphql(`
  mutation MobileRemoteAnswer($input: AgentThreadAnswerInput!) {
    answerAgentThreadRequest(input: $input) { ...MobileCommandReceipt }
  }
`);

const Stop = graphql(`
  mutation MobileRemoteStop($input: AgentThreadStopInput!) {
    stopAgentThread(input: $input) { ...MobileCommandReceipt }
  }
`);

function select<T, K extends keyof T>(result: HubResult<T>, key: K): HubResult<T[K]> {
  return result.ok ? { ok: true, value: result.value[key] } : result;
}

export type StartTarget = MobileRemoteTargetsQuery['agentThreadStartTargets'][number];
export type ControlOffer = NonNullable<MobileRemoteControlQuery['agentThreadControl']['offer']>;
export type LaunchReceipt = MobileLaunchReceiptFragment;
export type CommandReceipt = MobileCommandReceiptFragment;

export function remoteCodeApi(hub: Hub) {
  return {
    async targets() {
      return select(await hub.request(Targets, {}), 'agentThreadStartTargets');
    },
    async launch(launch: string) {
      return select(await hub.request(Launch, { launch }), 'agentThreadLaunch');
    },
    async command(command: string) {
      return select(await hub.request(Command, { command }), 'agentThreadCommand');
    },
    async control(thread: string) {
      const result = await hub.request(Control, { thread });
      if (!result.ok) return result;
      const view = result.value.agentThreadControl;
      return { ok: true as const, value: view.thread === thread ? view.offer ?? null : null };
    },
    async start(input: AgentThreadStartInput) {
      return select(await hub.request(Start, { input }), 'startAgentThread');
    },
    async prompt(input: AgentThreadPromptInput) {
      return select(await hub.request(Prompt, { input }), 'sendAgentThreadPrompt');
    },
    async answer(input: AgentThreadAnswerInput) {
      return select(await hub.request(Answer, { input }), 'answerAgentThreadRequest');
    },
    async stop(input: AgentThreadStopInput) {
      return select(await hub.request(Stop, { input }), 'stopAgentThread');
    },
  };
}

export type RemoteCodeApi = ReturnType<typeof remoteCodeApi>;
