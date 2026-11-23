import { toolsetLabel, type McpChange } from './permissions';

// `key` is what a write API key reaches; `signIn` is what only the operator's own sign-in reaches (MCP-003).
interface Consequences {
  readonly key: readonly string[];
  readonly signIn: readonly string[];
}

const CONSEQUENCES: Readonly<Record<string, Consequences>> = {
  devices: { key: ['send wake packets'], signIn: ['delete devices with their stored credentials'] },
  checks: { key: ['pause or delete checks', 'point HTTP checks at internal URLs'], signIn: [] },
  maintenance: { key: ['start or delete maintenance windows'], signIn: [] },
  sessions: { key: ['end live sessions'], signIn: [] },
  notifications: { key: ['silence checks by emptying their notification channels', 'send test notifications'], signIn: [] },
  web: { key: ['repoint web interfaces at other addresses', 'delete web interfaces'], signIn: [] },
  releases: { key: [], signIn: ['start a release', 'cancel builds'] },
  vpn: { key: [], signIn: ['approve a VPN route'] },
  retention: { key: [], signIn: ['cut every data retention period to the minimum'] },
  extensions: { key: [], signIn: ['set extensions on or off'] },
  sources: {
    key: ['pause or start a sync of a connected account'],
    signIn: [
      'disconnect an account and delete its records',
      'start a GitHub connection, which does nothing until you press Continue, Create and Install',
    ],
  },
  projects: { key: ['leave repositories out of Projects', 'confirm where a project runs'], signIn: [] },
  repositories: {
    key: [
      'make issues and comments on GitHub, and close or reopen issues',
      'copy text it read in a private repository into an issue or a comment in a public one',
      'set off a workflow of a repository that listens for issues or comments',
    ],
    signIn: [],
  },
};

export interface AllowQuestion {
  readonly title: string;
  readonly body: string;
  readonly lines: readonly string[];
  readonly confirmLabel: string;
}

export function consequencesOf(toolsets: readonly string[]): Consequences {
  const found = toolsets.map((toolset) => CONSEQUENCES[toolset]).filter((each) => each !== undefined);
  return { key: found.flatMap((each) => each.key), signIn: found.flatMap((each) => each.signIn) };
}

export function allowQuestion(change: Extract<McpChange, { type: 'bulk' }>, toolsets: readonly string[]): AllowQuestion {
  const scope = change.toolset === undefined ? 'every tool on this hub' : `every ${toolsetLabel(change.toolset)} tool`;
  const { key, signIn } = consequencesOf(change.toolset === undefined ? toolsets : [change.toolset]);
  const risky = key.length + signIn.length > 0;
  const lines = [
    key.length > 0 ? `With an API key that can write: ${key.join('; ')}.` : '',
    signIn.length > 0 ? `When anima mcp runs with your own sign-in, also: ${signIn.join('; ')}.` : '',
    risky ? 'The hub token still limits what an agent may do.' : '',
  ].filter((line) => line !== '');
  const effect = risky
    ? 'Text an agent reads can steer it, so it could then, without asking:'
    : 'This scope has no tool that changes the hub.';
  return {
    title: `Allow ${scope}?`,
    body: `Turns on ${scope}, read tools too, without asking per call. ${effect}`,
    lines,
    confirmLabel: change.toolset === undefined ? 'Allow every tool' : `Allow every ${toolsetLabel(change.toolset)} tool`,
  };
}
