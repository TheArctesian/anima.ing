/**
 * The chats of the timeline kit page (stage 3 of the T3 Code copy): each state of the timeline once, with made-up
 * entries, for the gallery. The time is fixed so the pictures do not change.
 */

import type { LiveEntry, OpenAsk } from '$lib/agent-threads/live/timeline';
import type { ToolCall, ToolKind } from '$lib/agent-threads/thread';

/** The time now of every scene: 09:01:12 on the day of the copy. */
export const NOW = new Date(Date.UTC(2026, 10, 21, 9, 1, 12));

const at = (seconds: number): Date => new Date(Date.UTC(2026, 10, 21, 9, 0, seconds));

function tool(kind: ToolKind, input: Record<string, unknown>, output: string | null, failed = false): ToolCall {
  const text = JSON.stringify(input);
  return {
    name: kind === 'command' ? 'Bash' : kind,
    kind,
    summary: text,
    input: text,
    patch: null,
    output,
    failed,
  };
}

const prompt = (text: string, seconds: number): LiveEntry => ({
  kind: 'prompt',
  at: at(seconds),
  text,
  images: 0,
});
const reply = (text: string, seconds: number): LiveEntry => ({
  kind: 'reply',
  at: at(seconds),
  text,
});
const call = (made: ToolCall, seconds: number): LiveEntry => ({
  kind: 'tool',
  at: at(seconds),
  tool: made,
  call: `call-${String(seconds)}`,
});
const thought = (text: string, seconds: number): LiveEntry => ({
  kind: 'reasoning',
  at: at(seconds),
  text,
});
const end = (seconds: number, stopped = false): LiveEntry => ({
  kind: 'turn_end',
  at: at(seconds),
  stopped,
});

const ANSWER = `The fold now hides the work of a finished turn.

## What changed

- \`turns.ts\` reads the entries as **turns**.
- \`words.ts\` gives each tool call one sentence.

\`\`\`ts
export function timer(milliseconds: number): string {
  const seconds = Math.max(0, Math.floor(milliseconds / 1000));
  return seconds < 60 ? \`\${String(seconds)}s\` : \`\${String(Math.floor(seconds / 60))}m \${String(seconds % 60)}s\`;
}
\`\`\`

Run \`bun run gates:changed\` before the push.`;

const WORK: readonly LiveEntry[] = [
  thought('The fold must hide every part but the last reply. Read the model first.', 1),
  call(tool('read', { file_path: '/w/apps/client/src/lib/code/timeline/turns.ts' }, 'export function turnsOf(…)'), 2),
  call(tool('search', { pattern: 'folded(' }, 'turns.ts:120\nTurnView.svelte:21'), 3),
  call(tool('command', { command: 'git status --short' }, ' M apps/client/src/lib/code/timeline/turns.ts'), 4),
  call(tool('edit', { file_path: '/w/apps/client/src/lib/code/timeline/TurnView.svelte' }, 'ok'), 5),
  call(tool('command', { command: 'bunx vitest run src/lib/code/timeline' }, ' Test Files  1 passed (1)\n      Tests  8 passed (8)'), 40),
];

export interface Scene {
  readonly entries: readonly LiveEntry[];
  readonly streaming?: string;
  readonly asks?: readonly OpenAsk[];
  readonly working?: boolean;
}

const LONG_PROMPT = Array.from(
  { length: 30 },
  (_, index) => `Step ${String(index + 1)}: check the part of the timeline that this line names, and say what you found.`,
).join('\n');

export const SCENES: Readonly<Record<string, Scene>> = {
  finished: {
    entries: [prompt('Fold the work of a finished turn, as T3 Code does.', 0), ...WORK, reply(ANSWER, 72), end(72)],
  },
  working: {
    entries: [
      prompt('Fold the work of a finished turn, as T3 Code does.', 0),
      ...WORK.slice(0, 4),
      call(tool('command', { command: 'bun run gates:changed' }, null), 50),
    ],
    streaming: 'I changed the fold. Now the gates run:\n\n- documents\n- lint',
    working: true,
  },
  approval: {
    entries: [prompt('Clean the build folder.', 0), call(tool('command', { command: 'ls build' }, 'index.html\n_app'), 2)],
    asks: [
      {
        request: 'r1',
        ask: {
          kind: 'command',
          tool: 'Claude Code',
          subject: 'rm -rf apps/client/build',
          diff: null,
          sessionRules: [],
          questions: [],
        },
      },
    ],
    working: true,
  },
  question: {
    entries: [prompt('Set up the release.', 0)],
    asks: [
      {
        request: 'r2',
        ask: {
          kind: 'question',
          tool: 'AskUserQuestion',
          subject: 'Which channel?',
          diff: null,
          sessionRules: [],
          questions: [
            {
              key: 'q',
              header: null,
              text: 'Which channel should the release go to: beta or stable?',
              choices: ['beta', 'stable'],
              many: false,
            },
          ],
        },
      },
    ],
    working: true,
  },
  stopped: {
    entries: [
      prompt('Run the whole test suite.', 0),
      call(tool('command', { command: 'cargo nextest run --workspace' }, null), 2),
      end(9, true),
      prompt('Run only the daemon tests instead.', 10),
      call(tool('command', { command: 'cargo nextest run -p anima-daemon' }, 'Summary 87 tests run: 87 passed'), 12),
      reply('The 87 tests of the daemon pass.', 31),
      end(31),
    ],
  },
  failed: {
    entries: [
      prompt('Build the client.', 0),
      call(tool('command', { command: 'bun run build' }, 'error: Cannot find module "./turns"', true), 3),
      reply('The build failed: `./turns` is missing. I will add it.', 4),
      end(4),
    ],
  },
  long: {
    entries: [prompt(LONG_PROMPT, 0), reply('Each step checked.', 20), end(20)],
  },
  empty: { entries: [] },
};
