import { describe, expect, it } from 'vitest';
import type { RunRow } from '$lib/agent-threads/live/run';
import { runEngine, type RunCalls } from './engine';
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
