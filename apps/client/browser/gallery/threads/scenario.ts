import type { Scenario } from './desktop-shell';

/**
 * The made-up threads of one computer that the gallery photographs: session files of Claude Code and Codex, and threads
 * that the runner drives, one in each state that the person meets. No name, folder or text comes from a real computer.
 */

const id = (n: number): string => `0190b000-0000-7000-8000-0000000000${String(n).padStart(2, '0')}`;

export const IDS = {
  askingFile: id(1),
  mayWait: id(2),
  yourTurn: id(3),
  writing: id(4),
  earlier: id(5),
  pinnedFile: id(6),
  archivedA: id(7),
  archivedB: id(8),
  asks: id(11),
  working: id(12),
  starting: id(13),
  talked: id(14),
} as const;

interface FileRow {
  readonly id: string;
  readonly program: 'claude_code' | 'codex';
  readonly title: string;
  readonly folder: string;
  readonly tail: string;
  readonly last: string;
}

/** A session file, as the shell lists it. */
const file = (row: FileRow) => ({ ...row, branch: 'main', model: row.program === 'codex' ? 'gpt-5.1-codex' : 'claude-opus-5-5', started: '@-180' });

const ROWS = [
  file({ id: IDS.mayWait, program: 'claude_code', title: 'Bump the Rust toolchain to 1.92', folder: '/Users/you/anima', tail: 'open_tool', last: '@-6' }),
  file({ id: IDS.yourTurn, program: 'codex', title: 'Review the sign-in flow', folder: '/Users/you/anima', tail: 'turn_ended', last: '@-14' }),
  file({ id: IDS.writing, program: 'claude_code', title: 'Write the release notes', folder: '/Users/you/notes', tail: 'open', last: '@-1' }),
  file({ id: IDS.earlier, program: 'claude_code', title: 'Tidy the device page', folder: '/Users/you/anima', tail: 'turn_ended', last: '@-2900' }),
  file({ id: IDS.pinnedFile, program: 'codex', title: 'Port the battery meter', folder: '/Users/you/anima', tail: 'turn_ended', last: '@-4400' }),
  file({ id: IDS.archivedA, program: 'claude_code', title: 'Spike: WebGPU charts', folder: '/Users/you/spikes', tail: 'turn_ended', last: '@-12000' }),
  file({ id: IDS.archivedB, program: 'codex', title: 'Try the new linter', folder: '/Users/you/spikes', tail: 'stopped', last: '@-15000' }),
];

interface RunRow {
  readonly id: string;
  readonly program: 'claude_code' | 'codex';
  readonly title: string;
  readonly state: string;
  readonly started: string;
  readonly mode?: string;
  readonly open?: readonly string[];
}

/** A thread that the runner drives, as the call `list` answers. */
const run = (row: RunRow) => ({
  folder: '/Users/you/anima',
  mode: 'full_access',
  model: row.program === 'codex' ? 'gpt-5.1-codex' : 'claude-opus-5-5',
  session: null,
  ended: null,
  open: [],
  ...row,
});

const RUNNING = [
  run({ id: IDS.asks, program: 'claude_code', title: 'Group the accounts on Agent usage', state: 'needs_you', started: '@-25', mode: 'accept_edits', open: ['req-1'] }),
  run({ id: IDS.working, program: 'codex', title: 'Fix the flaky batching test', state: 'working', started: '@-9' }),
  run({ id: IDS.starting, program: 'claude_code', title: 'Explain the merge train', state: 'starting', started: '@-0.7' }),
  run({ id: IDS.talked, program: 'claude_code', title: 'Why does the gates run stop at limits?', state: 'idle', started: '@-50' }),
];

const DIFF = [
  '--- a/apps/client/src/lib/agent-usage/view/AccountList.svelte',
  '+++ b/apps/client/src/lib/agent-usage/view/AccountList.svelte',
  '@@ -26,7 +26,9 @@',
  '   <h3>By account</h3>',
  '-  {#if accounts.length > 0}',
  '+  {#if accounts.length === 0}',
  '+    <p class="none">No account to show.</p>',
  '+  {:else}',
  '   <ul class="cards">',
].join('\n');

let seq = 0;
const update = (thread: string, change: Record<string, unknown>) => ({ thread, seq: ++seq, ...change });
const said = (thread: string, event: Record<string, unknown>) => update(thread, { change: 'said', event });
const prompt = (thread: string, text: string) => update(thread, { change: 'prompt', text, source: 'person' });
const message = (thread: string, text: string) => said(thread, { kind: 'message', text, parent: null });
/** A tool call and its result: `call` is its ID, then the name of the tool, its input and its output. */
const tool = (thread: string, [call, name, input, output]: readonly [string, string, string, string]) => [
  said(thread, { kind: 'tool_call', id: call, name, input }),
  said(thread, { kind: 'tool_output', id: call, output, failed: false }),
];
const ended = (thread: string) => said(thread, { kind: 'turn_ended', outcome: 'completed', detail: null });

const UPDATES = [
  // The thread whose program asks to change a file.
  prompt(IDS.asks, 'Group the machines that share one account on the page Agent usage, with one limit bar for each account.'),
  message(IDS.asks, 'I will read the page first, then change the list of accounts so that an empty list says why.'),
  ...tool(IDS.asks, ['c-1', 'Read', '{"file_path":"apps/client/src/lib/agent-usage/view/AccountList.svelte"}', 'export let accounts;\n…']),
  update(IDS.asks, {
    change: 'asked',
    request: 'req-1',
    detail: { id: 'r-1', kind: 'file_change', tool: 'Edit', subject: 'apps/client/src/lib/agent-usage/view/AccountList.svelte', diff: DIFF, session_rules: ['Edit(apps/client/**)'], questions: [] },
  }),
  // The thread that works now.
  prompt(IDS.working, 'The test api_005 fails under load. Find out why and make it stable.'),
  ...tool(IDS.working, ['c-2', 'shell', '{"command":["cargo","nextest","run","-p","anima-hub","api_005"]}', 'PASS [ 2.104s] anima-hub::it devices::fields::batching::api_005']),
  said(IDS.working, { kind: 'text_delta', text: 'The test passes alone. It counts statements while other tests run, so' }),
  // The thread that the person talked with: two turns, the second one ended.
  prompt(IDS.talked, 'Why does the gates run stop at limits?'),
  message(IDS.talked, 'The gate **limits** counts the code lines of each file. A file over 300 lines stops the run unless its entry in `quality/baseline.json` allows it.'),
  ...tool(IDS.talked, ['c-3', 'Bash', '{"command":"bun run quality:limits"}', 'apps/client/src/lib/agent-usage/screen.svelte.test.ts: file-lines 454 > 300\nlimits: 1 violation(s)']),
  message(IDS.talked, 'One test file grew to 454 lines when a formatter wrapped a long title. Keep the title on one line and the file is back under its baseline.'),
  ended(IDS.talked),
  prompt(IDS.talked, 'And the baseline: can it grow?'),
  message(
    IDS.talked,
    'No. `quality/baseline.json` only shrinks: an entry may go down, never up, and no entry is added by hand. When you make a file smaller, lower its entry in the same change.\n\nSo the way out of a failed limit is to split the file at a seam of its domain, run the gates, then lower the baseline.',
  ),
  ended(IDS.talked),
];

const LISTINGS = [
  { thread: IDS.pinnedFile, pinned: true, archived: false, title: null, removed: false },
  { thread: IDS.talked, pinned: true, archived: false, title: null, removed: false },
  { thread: IDS.archivedA, pinned: false, archived: true, title: null, removed: false },
  { thread: IDS.archivedB, pinned: false, archived: true, title: null, removed: false },
];

const THREADS: Record<string, Record<string, unknown>> = Object.fromEntries(
  ROWS.map((row) => [
    row.id,
    {
      ...row,
      prompts: 1,
      dropped: 0,
      entries: [
        { kind: 'prompt', at: '@-20', text: row.title, images: 0 },
        { kind: 'reply', at: '@-19', text: 'Done. The change is on the branch; the tests of the page pass.' },
        { kind: 'turn_end', at: '@-19', stopped: false },
      ],
    },
  ]),
);

/** The threads of the computer in each state, with the runner running. */
export const EVERY_STATE: Scenario = { rows: ROWS, threads: THREADS, running: RUNNING, listings: LISTINGS, updates: UPDATES };

/** A folder whose project files run commands with no question (THR-046). */
export const HOOKED_FOLDER = '/Users/you/anima-hooks';

/** The start dialog meets a folder with hooks, then a copy of the program that does not answer (TH97). */
export const STARTS: Scenario = {
  ...EVERY_STATE,
  notices: {
    [HOOKED_FOLDER]: [
      'The project files of this folder make Claude Code do these things with no question:',
      'Run the hook "bun run lint" after each change of a file (.claude/settings.json)',
      'Allow each command that starts with "cargo" (.claude/settings.local.json)',
    ],
  },
  startAnswer: { outcome: 'refused', why: { refusal: 'program_does_not_answer', answered: ['/opt/homebrew/bin/claude'] } },
};

/** The holder of the programs is older than the daemon (TH98): the words of the runner, word for word. */
export const OLDER_HOLDER: Scenario = {
  ...EVERY_STATE,
  holder:
    'The holder of the threads is older than the daemon, so the corrections of this build are not in force for its programs. It is replaced by itself when no thread runs (2 running now); stop them to replace it at once.',
};

/** The runner of this computer is not running: the page still lists the session files. */
export const RUNNER_DOWN: Scenario = { ...EVERY_STATE, running: [], listings: [], updates: [], runnerDown: true };
