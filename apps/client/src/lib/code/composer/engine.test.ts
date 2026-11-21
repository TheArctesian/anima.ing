import { describe, expect, it } from 'vitest';
import type { RunRow } from '$lib/agent-threads/live/run';
import { runEngine } from './engine';

const row: RunRow = { id: 'r1', program: 'claude_code', folder: '/work/anima', mode: 'accept_edits', model: null, state: 'idle', session: null, title: 'Fix the gate', started: new Date(0), ended: null, open: [] };
const sent: string[] = [];
const calls = {
  onSend: (text: string) => {
    sent.push(text);
    return Promise.resolve({ ok: false as const, words: 'The runner did not answer.' });
  },
  onInterrupt: () => Promise.resolve({ ok: false as const, words: 'No turn runs.' }),
};

describe('the engine of a driven chat', () => {
  it('is idle, running or ended by the state of the run', () => {
    expect(runEngine(row, calls).phase).toBe('idle');
    for (const state of ['working', 'starting', 'needs_you'] as const) expect(runEngine({ ...row, state }, calls).phase).toBe('running');
    const gone = runEngine({ ...row, state: 'stopped', ended: 'The program was closed.' }, calls);
    expect(gone.phase).toBe('ended');
    expect(gone.ended).toBe('The program was closed.');
    expect(runEngine({ ...row, state: 'stopped' }, calls).ended).toBe('The program of this chat ended.');
  });

  it('sends the prompt and gives the words of a failure', async () => {
    expect(await runEngine(row, calls).send({ prompt: 'Run the gates', attachments: [] })).toEqual({ ok: false, words: 'The runner did not answer.' });
    expect(sent).toEqual(['Run the gates']);
    expect(await runEngine(row, calls).interrupt()).toEqual({ ok: false, words: 'No turn runs.' });
  });

  it('says why for each part the runner lacks, and makes no call for it', async () => {
    const engine = runEngine(row, calls);
    for (const part of [engine.attach, engine.dictate, engine.queue, engine.pick]) expect(part.can).toBe(false);
    expect(engine.steer.can).toBe(true);
    expect((await engine.enqueue({ prompt: 'later', attachments: [] })).ok).toBe(false);
    expect(sent).toEqual(['Run the gates']);
  });

  it('shows the program, its model and its permissions as the one choice', () => {
    const { catalogue, choice } = runEngine(row, calls);
    expect(catalogue.providers.map((provider) => provider.name)).toEqual(['Claude Code']);
    expect(catalogue.models.map((model) => model.label)).toEqual(['Default model']);
    expect(choice.mode).toBe('auto-accept-edits');
    expect(runEngine({ ...row, model: 'opus', mode: 'full_access' }, calls).choice).toMatchObject({ model: 'opus', mode: 'full-access' });
  });
});
