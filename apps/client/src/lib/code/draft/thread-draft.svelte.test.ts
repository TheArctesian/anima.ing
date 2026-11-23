import { describe, expect, it } from 'vitest';
import type { CallResult } from '$lib/agent-threads/live/live.svelte';
import type { Action } from '$lib/agent-threads/live/run';
import { chosen, draftCatalogue, FIRST_CHOICE, MODELS_WAITING } from './choice';
import { noticeParts } from './notice';
import { addedFolder, draftProjects, headingOf } from './project';
import { ThreadDraft } from './thread-draft.svelte';

const MESSAGE = { prompt: 'Fix the gate', attachments: [] };

/** A runner that answers each call from `answers` in turn and keeps the calls. */
function runner(...answers: CallResult[]): { calls: Action[]; call: (action: Action) => Promise<CallResult> } {
  const calls: Action[] = [];
  return {
    calls,
    call: (action) => {
      calls.push(action);
      return Promise.resolve(answers.shift() ?? { ok: false, words: 'No answer.' });
    },
  };
}

const NOTHING: CallResult = { ok: true, outcome: { outcome: 'folder', notice: [] } };
const STARTED: CallResult = { ok: true, outcome: { outcome: 'started', thread: 'th-9' } };

describe('ThreadDraft', () => {
  it('starts the thread at the first send with the folder, the picked choice and the prompt', async () => {
    const draft = new ThreadDraft('/work/anima');
    const { calls, call } = runner(NOTHING, STARTED);
    const started: string[] = [];
    const engine = draft.engine(call, (id) => started.push(id), { state: 'listed', lists: { codex: [{ id: 'gpt-5', label: 'GPT-5', efforts: ['low', 'high'], defaultEffort: null }] } });
    await engine.choose({ ...engine.choice, provider: 'codex', model: 'gpt-5', effort: 'high', mode: 'approval-required' });
    const outcome = await draft.engine(call, (id) => started.push(id), MODELS_WAITING).send(MESSAGE);
    expect(outcome).toEqual({ ok: true });
    expect(calls).toEqual([
      { call: 'folder', folder: '/work/anima' },
      { call: 'start', program: 'codex', folder: '/work/anima', mode: 'ask', model: 'gpt-5', effort: 'high', prompt: 'Fix the gate' },
    ]);
    expect(started).toEqual(['th-9']);
  });

  it('shows what the folder runs by itself and starts only at the next send', async () => {
    const draft = new ThreadDraft('/work/anima');
    const { calls, call } = runner({ ok: true, outcome: { outcome: 'folder', notice: ['Claude Code runs these hooks:', 'npm test'] } }, STARTED);
    const engine = draft.engine(call, () => undefined, MODELS_WAITING);
    expect((await engine.send(MESSAGE)).ok).toBe(false);
    expect(draft.notice?.lines).toEqual(['Claude Code runs these hooks:', 'npm test']);
    expect(await engine.send(MESSAGE)).toEqual({ ok: true });
    expect(calls.map((each) => each.call)).toEqual(['folder', 'start']);
  });

  it('asks for a project, and reads the folder again after a refused start', async () => {
    const none = new ThreadDraft(null);
    expect(await none.engine(runner().call, () => undefined, MODELS_WAITING).send(MESSAGE)).toEqual({ ok: false, words: expect.stringContaining('Choose a project') as string });
    const draft = new ThreadDraft('/work/anima');
    const { calls, call } = runner(NOTHING, { ok: false, words: 'Over the limit.' }, NOTHING, STARTED);
    const engine = draft.engine(call, () => undefined, MODELS_WAITING);
    expect(await engine.send(MESSAGE)).toEqual({ ok: false, words: 'Over the limit.' });
    expect(await engine.send(MESSAGE)).toEqual({ ok: true });
    expect(calls.map((each) => each.call)).toEqual(['folder', 'start', 'folder', 'start']);
  });

  it('takes no file before the thread has started', async () => {
    const draft = new ThreadDraft('/work/anima');
    const engine = draft.engine(runner().call, () => undefined, MODELS_WAITING);
    expect(engine.attach.can).toBe(false);
    expect((await engine.send({ prompt: 'x', attachments: [{ id: 'a' } as never] })).ok).toBe(false);
  });
});

describe('the draft choice', () => {
  it('offers both programs by their marks, each with its default and then its listed models', () => {
    const { catalogue, choice } = draftCatalogue(FIRST_CHOICE, { state: 'listed', lists: { codex: [{ id: 'gpt-5', label: 'GPT-5', efforts: ['low', 'minimal'], defaultEffort: null }] } });
    expect(catalogue.providers.map((each) => `${each.name}:${each.icon}`)).toEqual(['Claude Code:claude', 'Codex:openai']);
    expect(catalogue.models.map((each) => each.label)).toEqual(['Claude Code default', 'Codex default', 'GPT-5']);
    expect(catalogue.models.find((each) => each.id === 'gpt-5')?.efforts.map((each) => each.id)).toEqual(['low']);
    expect(catalogue.models.map((each) => each.subLabel)).toEqual([undefined, undefined, undefined]);
    expect(catalogue.notice).toEqual({ words: 'Claude Code listed no models.', failed: false });
    expect(choice).toMatchObject({ provider: 'claude_code', model: 'default', mode: 'full-access' });
  });

  it("a program's default takes only the efforts of that program: Codex has no Max", () => {
    const efforts = draftCatalogue(FIRST_CHOICE, MODELS_WAITING).catalogue.models.map((each) => `${each.provider}: ${each.efforts.map((one) => one.label).join(', ')}`);
    expect(efforts).toEqual(['claude_code: Low, Medium, High, Extra high, Max', 'codex: Low, Medium, High, Extra high']);
  });

  it('says on a line of its own why the default stands alone, never in a row, and offers to ask again on a failure', () => {
    const notice = (models: Parameters<typeof draftCatalogue>[1]) => draftCatalogue(FIRST_CHOICE, models).catalogue.notice;
    expect(notice(MODELS_WAITING)).toEqual({ words: 'Models load when the programs answer.', failed: false });
    expect(notice({ state: 'failed', words: 'The runner is not running.' })).toEqual({ words: 'Models could not load: The runner is not running.', failed: true });
    const listed = { id: 'm', label: 'M', efforts: [], defaultEffort: null };
    expect(notice({ state: 'listed', lists: { claude_code: [listed], codex: [listed] } })).toBeUndefined();
    expect(draftCatalogue(FIRST_CHOICE, MODELS_WAITING).catalogue.models.every((each) => each.subLabel === undefined)).toBe(true);
  });

  it('keeps the default access when a draft in plan mode moves to Codex, which has none', () => {
    const plan = chosen(FIRST_CHOICE, { provider: 'claude_code', model: 'default', effort: null, speed: null, toggles: [], mode: 'plan' });
    expect(plan.mode).toBe('plan');
    expect(chosen(plan, { provider: 'codex', model: 'default', effort: 'low', speed: null, toggles: [], mode: 'plan' })).toEqual({ program: 'codex', model: null, effort: 'low', mode: 'full_access' });
  });
});

describe('the draft project', () => {
  it('names the heading by the project, asks for one, or asks to add one', () => {
    const projects = [{ folder: '/work/anima', name: 'anima', count: 2 }];
    expect(headingOf('/work/anima', projects)).toEqual({ kind: 'build', name: 'anima' });
    expect(headingOf(null, projects)).toEqual({ kind: 'choose' });
    expect(headingOf(null, [])).toEqual({ kind: 'add' });
  });

  it('takes an absolute folder for a new project and lists it once', () => {
    expect(addedFolder(' /work/site/ ')).toEqual({ ok: true, folder: '/work/site' });
    expect(addedFolder('site').ok).toBe(false);
    const known = [{ folder: '/work/anima', name: 'anima', count: 2 }, { folder: null, name: 'No folder', count: 1 }];
    expect(draftProjects(known, '/work/site').map((each) => each.folder)).toEqual(['/work/anima', '/work/site']);
    expect(draftProjects(known, '/work/anima')).toHaveLength(1);
  });
});

describe('the notice of a folder', () => {
  it('shows a warning before the heading as its own paragraph, and the lines under the heading as its list', () => {
    expect(noticeParts(['This folder holds Anima’s folder.', 'The project files of this folder make Claude Code do these things with no question:', 'Runs the command `x` at Stop.', 'Starts the MCP servers files.'])).toEqual([
      { words: 'This folder holds Anima’s folder.', items: [] },
      { words: 'The project files of this folder make Claude Code do these things with no question:', items: ['Runs the command `x` at Stop.', 'Starts the MCP servers files.'] },
    ]);
  });
});
