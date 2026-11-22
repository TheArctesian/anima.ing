import { describe, expect, it } from 'vitest';
import type { LiveEntry } from '$lib/agent-threads/live/timeline';
import type { ToolCall, ToolKind } from '$lib/agent-threads/thread';
import { spokenOf } from './spoken';
import { folded, turnsOf, type Now, type WorkEntry } from './turns';
import { duration, entryLabel, groupLabel, innerLabel, timer, turnLine } from './words';

const at = (seconds: number): Date => new Date(Date.UTC(2026, 10, 21, 9, 0, seconds));
const IDLE: Now = { streaming: '', asks: [], working: false };

function tool(kind: ToolKind, input: string, output: string | null = 'ok', failed = false): ToolCall {
  return {
    name: kind === 'command' ? 'Bash' : kind,
    kind,
    summary: input.split('\n', 1)[0] ?? '',
    input,
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
  call: `c${String(seconds)}`,
});
const end = (seconds: number, stopped = false): LiveEntry => ({
  kind: 'turn_end',
  at: at(seconds),
  stopped,
});

function entry(kind: ToolKind, input: string, extra: Partial<WorkEntry> = {}): WorkEntry {
  return {
    key: 'e',
    kind,
    at: null,
    tool: tool(kind, input),
    text: '',
    running: false,
    stopped: false,
    ...extra,
  };
}

describe('turns of a thread', () => {
  it('opens a turn at each prompt and groups the tool calls that follow each other', () => {
    const turns = turnsOf(
      [
        prompt('Fix it', 0),
        call(tool('read', '{"file_path":"/w/a.ts"}'), 1),
        call(tool('command', '{"command":"git status"}'), 2),
        reply('Looking', 3),
        call(tool('edit', '{"file_path":"/w/a.ts"}'), 4),
        reply('Done', 72),
        end(72),
      ],
      IDLE,
    );
    expect(turns).toHaveLength(1);
    expect(turns[0]?.parts.map((part) => part.kind)).toEqual(['work', 'reply', 'work', 'reply']);
    const parts = turns[0] === undefined ? null : folded(turns[0]);
    expect(parts?.hidden.map((part) => part.kind)).toEqual(['work', 'reply', 'work']);
    expect(parts?.answer?.kind).toBe('reply');
  });

  it('keeps the reply that streams and the open requests in the running turn, and folds nothing there', () => {
    const ask = {
      request: 'r1',
      ask: {
        kind: 'command' as const,
        tool: 'Bash',
        subject: 'rm -rf build',
        diff: null,
        sessionRules: [],
        questions: [],
      },
    };
    const turns = turnsOf([prompt('Go', 0), call(tool('command', '{"command":"ls"}', null), 1)], { streaming: 'Half a sen', asks: [ask], working: true });
    const last = turns.at(-1);
    expect(last?.running).toBe(true);
    expect(last?.parts.map((part) => part.kind)).toEqual(['work', 'reply', 'ask']);
    const work = last?.parts[0];
    expect(work?.kind === 'work' && work.entries[0]?.running).toBe(true);
    expect(last === undefined ? null : folded(last).hidden).toEqual([]);
  });

});

describe('turns that stop or steer', () => {
  it('keeps the key of each turn when older entries go, and opens no turn for a repeated end', () => {
    const all = [prompt('One', 0), reply('A', 1), end(1), prompt('Two', 2), reply('B', 3), end(3), end(4)];
    const before = turnsOf(all, IDLE);
    const after = turnsOf(all.slice(3), { ...IDLE, dropped: 3 });
    expect(before.map((turn) => turn.key)).toHaveLength(2);
    expect(after.map((turn) => turn.key)).toEqual([before[1]?.key]);
  });

  it('gives the requests a turn when the program works on a turn with no entry yet', () => {
    const ask = { request: 'r', ask: { kind: 'command' as const, tool: 'Bash', subject: 'ls', diff: null, sessionRules: [], questions: [] } };
    const turns = turnsOf([prompt('Go', 0), end(1)], { streaming: '', asks: [ask], working: true });
    expect(turns.at(-1)?.parts.map((part) => part.kind)).toEqual(['ask']);
  });

  it('marks a tool call cut by a stop as stopped', () => {
    const [turn] = turnsOf([prompt('Go', 0), call(tool('command', '{"command":"sleep 9"}', null), 1), end(5, true)], IDLE);
    const work = turn?.parts[0];
    expect(work?.kind === 'work' ? work.entries[0] : null).toMatchObject({
      running: false,
      stopped: true,
    });
    expect(turn?.stopped).toBe(true);
  });

  it('marks a prompt sent while a turn runs as a steer', () => {
    const turns = turnsOf([prompt('Go', 0), call(tool('command', '{"command":"sleep 9"}', null), 1), prompt('Use the other folder', 2)], {
      ...IDLE,
      working: true,
    });
    expect(turns.map((turn) => turn.prompt?.steer)).toEqual([false, true]);
    expect(turns.map((turn) => turn.running)).toEqual([false, true]);
    const cut = turns[0]?.parts[0];
    expect(cut?.kind === 'work' ? cut.entries[0] : null).toMatchObject({ running: false, stopped: true });
  });
});

describe('words of the work log', () => {
  it('says one past-tense sentence for each kind and the running form while it runs', () => {
    expect(entryLabel(entry('command', '{"command":"/usr/bin/git status --short"}'))).toBe('Ran git');
    expect(entryLabel(entry('command', '{"command":"bun test"}', { running: true }))).toBe('Running bun');
    expect(
      entryLabel(
        entry('command', '{"command":"false"}', {
          tool: tool('command', '{"command":"false"}', 'x', true),
        }),
      ),
    ).toBe('Failed false');
    expect(entryLabel(entry('command', '{}'))).toBe('Ran command');
    expect(entryLabel(entry('read', '{"file_path":"/w/src/turns.ts"}'))).toBe('Read turns.ts');
    expect(entryLabel(entry('read', '{"offset":3}'))).toBe('Read file');
    expect(entryLabel(entry('edit', '{"file_path":"/w/Menu.svelte"}', { running: true }))).toBe('Changing Menu.svelte');
    expect(entryLabel(entry('search', '{"pattern":"ToolKind"}'))).toBe('Searched code for ToolKind');
    expect(innerLabel(entry('command', '{"command":"git status --short"}'))).toBe('git status --short');
  });

  it('sums a group in at most two clauses', () => {
    const read = entry('read', '{}');
    const ran = entry('command', '{}');
    expect(groupLabel([read, read, read, ran, ran])).toBe('Read 3 files and ran 2 commands');
    expect(groupLabel([read, ran, entry('edit', '{}'), entry('web', '{}')])).toBe('Read 1 file, ran 1 command, and performed 2 other actions');
    expect(groupLabel([ran])).toBe('Ran 1 command');
  });
});

describe('times of a turn', () => {
  it('writes the timer and the duration as T3 Code writes them', () => {
    expect([timer(12_400), timer(72_000), timer(3_723_000)]).toEqual(['12s', '1m 12s', '1h 2m 3s']);
    expect([duration(0.2), duration(850), duration(4_250), duration(9_000), duration(42_000), duration(59_600), duration(72_000)]).toEqual(['1ms', '850ms', '4.3s', '9s', '42s', '1m 0s', '1m 12s']);
  });

  it('says Working while the turn runs and Worked or You stopped after it ends', () => {
    const [done] = turnsOf([prompt('Go', 0), reply('Done', 72), end(72)], IDLE);
    const [stopped] = turnsOf([prompt('Go', 0), end(9, true)], IDLE);
    const [running] = turnsOf([prompt('Go', 0)], { ...IDLE, working: true });
    if (done === undefined || stopped === undefined || running === undefined) throw new Error('a turn');
    expect(turnLine(done, at(99))).toBe('Worked for 1m 12s');
    expect(turnLine(stopped, at(99))).toBe('You stopped after 9s');
    // Fidelity 7: only the newest turn says that the person stopped it.
    expect(turnLine(stopped, at(99), false)).toBe('Worked for 9s');
    expect(turnLine(running, at(12))).toBe('Working for 12s');
  });

  it('says Worked with no duration when the turn holds no time between its start and its end', () => {
    const [same] = turnsOf([prompt('Go', 5), reply('Done', 5), end(5)], IDLE);
    if (same === undefined) throw new Error('a turn');
    expect(turnLine(same, at(99))).toBe('Worked');
  });
});

describe('the words of the live region', () => {
  it('says the stop of a turn, and the first sentence of a reply at most 160 characters', () => {
    const [stopped] = turnsOf([prompt('Go', 0), end(9, true)], IDLE);
    const long = 'a'.repeat(400);
    if (stopped === undefined) throw new Error('a turn');
    expect(spokenOf([stopped])?.words).toBe('Stopped.');
    expect(spokenOf(turnsOf([prompt('Go', 0), reply(long, 3), end(3)], IDLE))?.words).toHaveLength('Agent replied: '.length + 160);
  });
});
