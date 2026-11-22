import { describe, expect, it } from 'vitest';
import type { RunRow } from '$lib/agent-threads/live/run';
import { runEngine, type QueueOfRow, type RunCalls } from './engine';
import { HeldQueue } from './queue/held.svelte';

const row: RunRow = { id: 'r1', program: 'claude_code', folder: '/work/anima', mode: 'accept_edits', model: null, state: 'idle', session: null, title: 'Fix the gate', started: new Date(0), ended: null, open: [] };
const sent: string[] = [];
const calls = {
  onSend: (text: string) => {
    sent.push(text);
    return Promise.resolve({ ok: false as const, words: 'The runner did not answer.' });
  },
  onInterrupt: () => Promise.resolve({ ok: false as const, words: 'No turn runs.' }),
};

const engineOf = (of: RunRow, by: RunCalls, queue = new HeldQueue()) => runEngine(of, by, queue);

describe('the engine of a driven chat', () => {
  it('is idle, running or ended by the state of the run', () => {
    expect(engineOf(row, calls).phase).toBe('idle');
    for (const state of ['working', 'starting', 'needs_you'] as const) expect(engineOf({ ...row, state }, calls).phase).toBe('running');
    const gone = engineOf({ ...row, state: 'stopped', ended: 'The program was closed.' }, calls);
    expect(gone.phase).toBe('ended');
    expect(gone.ended).toBe('The program was closed.');
    expect(engineOf({ ...row, state: 'stopped' }, calls).ended).toBe('The program of this chat ended.');
  });

  it('sends the prompt and gives the words of a failure', async () => {
    expect(await engineOf(row, calls).send({ prompt: 'Run the gates', attachments: [] })).toEqual({ ok: false, words: 'The runner did not answer.' });
    expect(sent).toEqual(['Run the gates']);
    expect(await engineOf(row, calls).interrupt()).toEqual({ ok: false, words: 'No turn runs.' });
  });

  it('says why for each part the runner lacks', () => {
    const engine = engineOf(row, calls);
    for (const part of [engine.attach, engine.dictate, engine.pick, engine.sources.search]) expect(part?.can).toBe(false);
    expect(engine.steer.can).toBe(true);
    expect(engine.queue.can).toBe(true);
  });

  it('queues in the window while a turn runs, and sends at once when none does', async () => {
    const queue = new HeldQueue();
    const running = engineOf({ ...row, state: 'working' }, calls, queue);
    expect(await running.enqueue({ prompt: 'later', attachments: [] })).toEqual({ ok: true });
    expect(queue.messages.map((message) => message.prompt)).toEqual(['later']);
    expect(sent).toEqual(['Run the gates']);
    // The turn ended between the press and the call: nothing would send a message queued now.
    expect((await engineOf(row, calls, queue).enqueue({ prompt: 'now', attachments: [] })).ok).toBe(false);
    expect(sent).toEqual(['Run the gates', 'now']);
    expect(queue.messages).toHaveLength(1);
  });

  it('makes the queue wait when the person stops the turn, unless the stop failed', async () => {
    const queue = new HeldQueue();
    queue.add('after');
    const stops = { ...calls, onInterrupt: () => Promise.resolve({ ok: true as const, outcome: { outcome: 'ok' as const } }) };
    await engineOf({ ...row, state: 'working' }, stops, queue).interrupt();
    expect(queue.waits).toMatch(/^You stopped the turn/);
    // A send that did not go starts no turn: the queue still waits.
    await engineOf(row, calls, queue).send({ prompt: 'go on', attachments: [] });
    expect(queue.waits).toMatch(/^You stopped the turn/);
    await engineOf(row, { ...calls, onSend: () => Promise.resolve({ ok: true as const, outcome: { outcome: 'ok' as const } }) }, queue).send({ prompt: 'go on', attachments: [] });
    expect(queue.waits).toBeNull();
    await engineOf({ ...row, state: 'working' }, calls, queue).interrupt();
    expect(queue.waits).toBeNull();
  });

});

describe('the lists and the choice of a driven chat', () => {
  it('searches the folder of the chat through the runner when the page gives its file list', async () => {
    const asked: [string, number][] = [];
    const onFiles = (query: string, most: number) => {
      asked.push([query, most]);
      return Promise.resolve(query === 'no' ? { ok: false as const, words: 'The runner refused.' } : { ok: true as const, paths: [{ path: 'src/gate.ts', folder: false }], more: false });
    };
    const { sources } = engineOf(row, { ...calls, onFiles });
    expect(sources.search?.can).toBe(true);
    expect(await sources.files('gate', 80)).toEqual([{ path: 'src/gate.ts', folder: false }]);
    await expect(sources.files('no', 80)).rejects.toThrow('The runner refused.');
    await sources.files('x'.repeat(300), 80);
    expect(asked.map(([query, most]) => [query.length, most])).toEqual([[4, 80], [2, 80], [64, 80]]);
  });

  it('shows the program, its model and its permissions as the one choice', () => {
    const { catalogue, choice } = engineOf(row, calls);
    expect(catalogue.providers.map((provider) => provider.name)).toEqual(['Claude Code']);
    expect(catalogue.models.map((model) => model.label)).toEqual(['Default model']);
    expect(choice.mode).toBe('auto-accept-edits');
    expect(engineOf({ ...row, model: 'opus', mode: 'full_access' }, calls).choice).toMatchObject({ model: 'opus', mode: 'full-access' });
    const planning = engineOf({ ...row, mode: 'plan' }, calls);
    expect(planning.catalogue.modes.find((mode) => mode.id === planning.choice.mode)?.label).toBe('Plan');
  });
});

describe('the engine over the queue of the runner', () => {
  const went = { ok: true as const, outcome: { outcome: 'ok' as const } };
  const acts: unknown[] = [];
  const onQueue = (act: unknown) => {
    acts.push(act);
    return Promise.resolve(went);
  };
  const working: RunRow & QueueOfRow = { ...row, state: 'working', queued: [{ id: 'p1', text: 'then push' }] };

  it('shows the prompts the runner holds, and makes each change through the runner', async () => {
    const engine = runEngine(working, { ...calls, onQueue }, null);
    expect(engine.queue.can).toBe(true);
    expect(engine.queued).toEqual([{ id: 'p1', prompt: 'then push', saving: false }]);
    await engine.enqueue({ prompt: 'then test', attachments: [] });
    await engine.requeue('p1', { prompt: 'then push the branch', attachments: [] });
    await engine.unqueue('p1');
    await engine.sendQueued('p1');
    expect(acts).toEqual([{ call: 'queue', text: 'then test' }, { call: 'requeue', queued: 'p1', text: 'then push the branch' }, { call: 'unqueue', queued: 'p1' }, { call: 'send_queued', queued: 'p1' }]);
  });

  it('sends at once what is queued when no turn runs, and says why the queue waits', async () => {
    const before = sent.length;
    await runEngine({ ...working, state: 'idle' }, { ...calls, onQueue }, null).enqueue({ prompt: 'now', attachments: [] });
    expect(sent.slice(before)).toEqual(['now']);
    const waits = (lastTurn: QueueOfRow['lastTurn'], state: RunRow['state'] = 'idle') => runEngine({ ...working, state, lastTurn }, { ...calls, onQueue }, null).queueWaits;
    expect(waits('interrupted')).toMatch(/^The last turn was stopped/);
    expect(waits('failed')).toMatch(/^The last turn failed/);
    expect(waits('done')).toBeNull();
    expect(waits('failed', 'working')).toBeNull();
    const none: RunRow & QueueOfRow = { ...row, lastTurn: 'failed' };
    expect(runEngine(none, { ...calls, onQueue }, null).queueWaits).toBeNull();
  });

  it('has no queue when the page gives neither the calls of the runner nor a queue of the window', async () => {
    const engine = runEngine(working, calls, null);
    expect(engine.queue.can).toBe(false);
    expect((await engine.enqueue({ prompt: 'x', attachments: [] })).ok).toBe(false);
  });
});
