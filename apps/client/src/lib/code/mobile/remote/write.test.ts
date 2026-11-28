// @vitest-environment jsdom
import { mount, unmount } from 'svelte';
import { afterEach, expect, it, vi } from 'vitest';
import { fakeHub } from '$lib/devices/hub.fixture';
import type { Hub } from '$lib/hub/hub';
import MobileCode from '../MobileCode.svelte';

const DEVICE = 'device-a';
const THREAD = 'thread-a';
const PROCESS = 'process-a';
const STOP = { kind: 'interrupt', process: PROCESS, turn: 'turn-a' };
const REQUEST = { kind: 'approval', id: 'approval-a', subject: 'Run the tests?', decisions: ['allow_once', 'decline'] };
const QUESTION = {
  kind: 'question', id: 'question-a', subject: 'Test settings',
  questions: [{ key: 'scope', text: 'Which checks?', choices: [{ id: 'fast', label: 'Quick checks' }], many: false, freeText: true }],
};
const TARGET = { device: DEVICE, project: 'project-a', folder: '/work/project', program: 'codex', expiresAt: '2099-01-01T00:00:00Z' };
const STARTED = { state: 'started', binding: 'waiting', thread: null, firstCommand: 'first-a', cause: null };

let url: URL;
let hub: Hub;
let app: ReturnType<typeof mount> | undefined;
vi.mock('$app/state', () => ({ page: { get url() { return url; } } }));
vi.mock('$lib/hub/context', () => ({ useHub: () => hub }));

function open(answers: Record<string, unknown>, search = '', endpoint = 'https://hub.test/graphql') {
  url = new URL(`https://hub.test/code${search}`);
  answers['MobileRemoteOperator'] ??= { me: { id: 'operator-a' } };
  const mock = fakeHub(answers);
  hub = { ...mock.hub, address: { ...mock.hub.address, http: endpoint } };
  vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible');
  app = mount(MobileCode, { target: document.body });
  return mock.requests;
}

function button(text: string): HTMLButtonElement {
  const target = [...document.querySelectorAll('button')].find((element) => element.textContent?.trim() === text);
  if (target === undefined) throw new Error(`Missing button: ${text}`);
  return target;
}

function select(index: number, option: number): void {
  const field = document.querySelectorAll('select')[index];
  if (field === undefined) throw new Error('Missing selection');
  field.selectedIndex = option;
  field.dispatchEvent(new Event('change', { bubbles: true }));
}

function fill(text: string): void {
  const field = document.querySelector('textarea');
  if (field === null) throw new Error('Missing text field');
  field.value = text;
  field.dispatchEvent(new Event('input', { bubbles: true }));
}

function inputs(requests: ReturnType<typeof open>, name: string) {
  return requests.filter(({ operation }) => operation === name).map(({ variables }) => variables['input']);
}

async function leave(): Promise<void> {
  if (app !== undefined) await unmount(app);
  app = undefined;
}

afterEach(async () => {
  await leave();
  localStorage.clear();
  vi.restoreAllMocks();
});

it('retries the same start and only offers to open the thread after the hub binds it', async () => {
  const answers: Record<string, unknown> = {
    MobileThreadInbox: { agentThreads: { nodes: [], pageInfo: { hasNextPage: false } } },
    MobileRemoteTargets: { agentThreadStartTargets: [TARGET] },
    MobileRemoteStart: { code: 'unreachable', message: 'The response was lost.' },
  };
  const requests = open(answers);
  await vi.waitFor(() => { button('New thread'); });
  button('New thread').click();
  await vi.waitFor(() => { expect(document.querySelector('select')?.textContent).toContain(TARGET.folder); });
  select(0, 1);
  select(1, 1);
  fill('Run the checkout tests');
  await vi.waitFor(() => { expect(button('Start thread').disabled).toBe(false); });
  button('Start thread').click();
  await vi.waitFor(() => { expect(document.body.textContent).toContain('The response was lost.'); });

  const [initial] = inputs(requests, 'MobileRemoteStart');
  expect(initial).toMatchObject({
    launch: expect.stringMatching(/^[a-f0-9-]{36}$/u), device: DEVICE, project: 'project-a', folder: TARGET.folder,
    program: 'codex', access: 'ask', prompt: 'Run the checkout tests',
  });
  answers['MobileRemoteStart'] = { startAgentThread: STARTED };
  button('Retry same start').click();
  await vi.waitFor(() => { expect(document.body.textContent).toContain('Waiting for it to appear on the hub.'); });
  expect(inputs(requests, 'MobileRemoteStart')).toEqual([initial, initial]);
  expect(document.querySelector('a[href*="thread="]')).toBeNull();

  answers['MobileRemoteLaunch'] = { agentThreadLaunch: { ...STARTED, binding: 'bound', thread: THREAD } };
  window.dispatchEvent(new Event('online'));
  await vi.waitFor(() => { expect(document.querySelector(`a[href="/code?thread=${THREAD}"]`)?.textContent).toBe('Open thread'); });
});

function conversation(offer: unknown): Record<string, unknown> {
  return {
    MobileThreadSnapshot: {
      agentThread: {
        summary: { id: THREAD, deviceId: DEVICE, program: 'codex', state: 'needs_you', title: 'Check checkout', repository: null },
        entries: [{ sequence: '1', content: { change: 'asked', request: 'approval-a', detail: { kind: 'command', subject: 'Run tests' } } }],
        lastSequence: '1',
      },
    },
    MobileRemoteControl: { agentThreadControl: { thread: THREAD, offer } },
  };
}

it('keeps an authorized desk thread read-only when the server offers no control', async () => {
  const requests = open(conversation(null), `?thread=${THREAD}`);
  await vi.waitFor(() => {
    expect(requests).toContainEqual({ operation: 'MobileRemoteControl', variables: { thread: THREAD } });
    expect(document.body.textContent).toContain('Open the thread on its computer to respond.');
  });
  expect(document.querySelector('textarea')).toBeNull();
  expect([...document.querySelectorAll('button')].map((element) => element.textContent?.trim()))
    .not.toContain('Stop the turn');
});

it('binds each answer and stop to the live offer while keeping a follow-up pending', async () => {
  const offer = { process: PROCESS, revision: 5, actions: ['prompt', 'answer', 'stop'], requests: [REQUEST, QUESTION], stopTargets: [STOP] };
  const recorded = { state: 'recorded', cause: null };
  const requests = open({
    ...conversation(offer),
    MobileRemotePrompt: { sendAgentThreadPrompt: { state: 'waiting', cause: null } },
    MobileRemoteAnswer: { answerAgentThreadRequest: recorded },
    MobileRemoteStop: { stopAgentThread: recorded },
  }, `?thread=${THREAD}`);
  await vi.waitFor(() => { expect(document.querySelector('textarea')).not.toBeNull(); });
  const followUp = document.querySelectorAll('textarea')[1];
  if (followUp === undefined) throw new Error('Missing follow-up field');
  followUp.value = 'Compare the fixtures';
  followUp.dispatchEvent(new Event('input', { bubbles: true }));
  await vi.waitFor(() => { expect(button('Send follow-up').disabled).toBe(false); });
  button('Send follow-up').click();
  button('Decline').click();
  await vi.waitFor(() => { expect(button('Decline').disabled).toBe(true); });
  const quickChecks = [...document.querySelectorAll('label')].find((label) => label.textContent?.trim() === 'Quick checks');
  quickChecks?.querySelector('input')?.click();
  button('Send answer').click();
  button('Stop the turn').click();

  await vi.waitFor(() => {
    expect(inputs(requests, 'MobileRemotePrompt')).toHaveLength(1);
    expect(inputs(requests, 'MobileRemoteAnswer')).toHaveLength(2);
    expect(inputs(requests, 'MobileRemoteStop')).toHaveLength(1);
  });
  const [prompt] = inputs(requests, 'MobileRemotePrompt');
  const sentAnswers = inputs(requests, 'MobileRemoteAnswer');
  const [stop] = inputs(requests, 'MobileRemoteStop');
  expect(prompt).toMatchObject({ thread: THREAD, prompt: 'Compare the fixtures' });
  expect(sentAnswers).toMatchObject([
    { thread: THREAD, process: PROCESS, revision: 5, request: 'approval-a', answer: { kind: 'approval', decision: 'decline' } },
    { thread: THREAD, process: PROCESS, revision: 5, request: 'question-a', answer: { kind: 'question', answers: [{ key: 'scope', values: [{ kind: 'choice', id: 'fast' }] }] } },
  ]);
  expect(stop).toMatchObject({ thread: THREAD, target: STOP });
  expect(new Set([prompt, ...sentAnswers, stop].map((input) => (input as { command: string }).command)).size).toBe(4);
});


it('retains an ambiguous start through later auth refusal and restores only its receipt in the original scope', async () => {
  const answers: Record<string, unknown> = {
    MobileThreadInbox: { agentThreads: { nodes: [], pageInfo: { hasNextPage: false } } },
    MobileRemoteTargets: { agentThreadStartTargets: [TARGET] },
    MobileRemoteStart: { code: 'unreachable', message: 'The response was lost.' },
  };
  const requests = open(answers);
  await vi.waitFor(() => { button('New thread'); });
  button('New thread').click();
  await vi.waitFor(() => { expect(document.querySelector('select')?.textContent).toContain(TARGET.folder); });
  select(0, 1);
  select(1, 1);
  fill('Sensitive synthetic prompt');
  await vi.waitFor(() => { expect(button('Start thread').disabled).toBe(false); });
  button('Start thread').click();
  await vi.waitFor(() => { button('Retry same start'); });

  answers['MobileRemoteStart'] = { code: 'UNAUTHENTICATED', message: 'Sign in again.' };
  button('Retry same start').click();
  await vi.waitFor(() => { expect(document.body.textContent).toContain('Sign in again.'); });
  expect(button('Start thread').disabled).toBe(true);
  const [first, retry] = inputs(requests, 'MobileRemoteStart');
  expect(retry).toEqual(first);
  const { launch } = first as { launch: string };
  expect(Object.values(localStorage)).toEqual([launch]);
  expect(Object.keys(localStorage).join()).not.toContain('Sensitive synthetic prompt');
  await leave();

  for (const [account, endpoint] of [
    ['operator-b', 'https://hub.test/graphql'],
    ['operator-a', 'https://other.test/graphql'],
  ] as const) {
    const other = open({ ...answers, MobileRemoteOperator: { me: { id: account } } }, '', endpoint);
    await vi.waitFor(() => { button('New thread'); });
    button('New thread').click();
    await vi.waitFor(() => { expect(other.some(({ operation }) => operation === 'MobileRemoteTargets')).toBe(true); });
    expect(other.some(({ operation }) => operation === 'MobileRemoteLaunch')).toBe(false);
    await leave();
  }

  answers['MobileRemoteLaunch'] = {
    agentThreadLaunch: { ...STARTED, launch, state: 'reconciling', binding: 'unavailable', cause: 'publication_timeout' },
  };
  const restored = open(answers);
  await vi.waitFor(() => { button('Check previous start'); });
  button('Check previous start').click();
  await vi.waitFor(() => { expect(document.body.textContent).toContain('Checking whether the computer started the thread.'); });
  expect(restored).toContainEqual({ operation: 'MobileRemoteLaunch', variables: { launch } });
  expect(button('Start thread').disabled).toBe(true);
  expect(document.body.textContent).not.toContain('Retry same start');

  answers['MobileRemoteLaunch'] = {
    agentThreadLaunch: { ...STARTED, launch, binding: 'unavailable', cause: 'publication_timeout' },
  };
  button('Check start status').click();
  await vi.waitFor(() => { expect(document.body.textContent).toContain('The computer started the thread.'); });
  expect(inputs(restored, 'MobileRemoteStart')).toEqual([]);
  expect(document.querySelector('a[href*="thread="]')).toBeNull();
  button('Start a different thread').click();
  await vi.waitFor(() => { expect(button('Start thread').disabled).toBe(true); });
  expect(document.querySelector('textarea')?.value).toBe('');
});

it('looks up an ambiguous command by its original ID when the thread reopens', async () => {
  const offer = { process: PROCESS, revision: 5, actions: ['prompt'], requests: [], stopTargets: [] };
  const answers: Record<string, unknown> = {
    ...conversation(offer),
    MobileRemotePrompt: { code: 'unreachable', message: 'The command response was lost.' },
  };
  const original = open(answers, `?thread=${THREAD}`);
  await vi.waitFor(() => { expect(document.querySelector('textarea')).not.toBeNull(); });
  fill('Follow-up payload kept in memory only');
  await vi.waitFor(() => { expect(button('Send follow-up').disabled).toBe(false); });
  button('Send follow-up').click();
  await vi.waitFor(() => { expect(document.body.textContent).toContain('The command response was lost.'); });
  const [first] = inputs(original, 'MobileRemotePrompt');
  const { command } = first as { command: string };
  expect(Object.values(localStorage)).toEqual([command]);
  await leave();

  answers['MobileRemoteCommand'] = { agentThreadCommand: { command, state: 'delivered', cause: null } };
  const restored = open(answers, `?thread=${THREAD}`);
  await vi.waitFor(() => { expect(document.body.textContent).toContain('Sent to the computer.'); });
  expect(restored).toContainEqual({ operation: 'MobileRemoteCommand', variables: { command } });
  expect(inputs(restored, 'MobileRemotePrompt')).toEqual([]);
  expect(button('Send follow-up').disabled).toBe(true);
  expect(document.querySelector('textarea')?.value).toBe('');
});


it('resubmits an unknown start only after re-entry and uses its saved ID', async () => {
  const answers: Record<string, unknown> = {
    MobileThreadInbox: { agentThreads: { nodes: [], pageInfo: { hasNextPage: false } } },
    MobileRemoteTargets: { agentThreadStartTargets: [TARGET] },
    MobileRemoteStart: { code: 'unreachable', message: 'The response was lost.' },
  };
  const original = open(answers);
  await vi.waitFor(() => { button('New thread'); });
  button('New thread').click();
  await vi.waitFor(() => { expect(document.querySelector('select')?.textContent).toContain(TARGET.folder); });
  select(0, 1);
  select(1, 1);
  fill('Rebuild the fixtures');
  await vi.waitFor(() => { expect(button('Start thread').disabled).toBe(false); });
  button('Start thread').click();
  await vi.waitFor(() => { button('Retry same start'); });
  const [first] = inputs(original, 'MobileRemoteStart');
  const { launch } = first as { launch: string };
  await leave();

  answers['MobileRemoteLaunch'] = { code: 'INTERNAL', reason: 'LAUNCH_UNKNOWN', message: 'Unknown.' };
  answers['MobileRemoteStart'] = { startAgentThread: { ...STARTED, launch } };
  const restored = open(answers);
  await vi.waitFor(() => { button('Check previous start'); });
  button('Check previous start').click();
  await vi.waitFor(() => { expect(document.querySelector('select')?.textContent).toContain(TARGET.folder); });
  expect(restored).toContainEqual({ operation: 'MobileRemoteLaunch', variables: { launch } });
  expect(inputs(restored, 'MobileRemoteStart')).toEqual([]);
  expect(button('Resubmit start').disabled).toBe(true);
  expect(document.querySelector('textarea')?.value).toBe('');
  select(0, 1);
  select(1, 1);
  fill('Rebuild the fixtures');
  await vi.waitFor(() => { expect(button('Resubmit start').disabled).toBe(false); });
  button('Resubmit start').click();
  await vi.waitFor(() => { expect(inputs(restored, 'MobileRemoteStart')).toEqual([first]); });
});

it('rechecks the holder offer before resubmitting an unknown follow-up under the original ID', async () => {
  const offer = { process: PROCESS, revision: 5, actions: ['prompt'], requests: [], stopTargets: [] };
  const answers: Record<string, unknown> = {
    ...conversation(offer),
    MobileRemotePrompt: { code: 'unreachable', message: 'The command response was lost.' },
  };
  const original = open(answers, `?thread=${THREAD}`);
  await vi.waitFor(() => { expect(document.querySelector('textarea')).not.toBeNull(); });
  fill('Compare fixture counts');
  await vi.waitFor(() => { expect(button('Send follow-up').disabled).toBe(false); });
  button('Send follow-up').click();
  await vi.waitFor(() => { button('Retry same follow-up'); });
  const [first] = inputs(original, 'MobileRemotePrompt');
  const { command } = first as { command: string };
  await leave();

  answers['MobileRemoteCommand'] = { code: 'INTERNAL', reason: 'COMMAND_UNKNOWN', message: 'Unknown.' };
  answers['MobileRemotePrompt'] = { sendAgentThreadPrompt: { command, state: 'waiting', cause: null } };
  const restored = open(answers, `?thread=${THREAD}`);
  await vi.waitFor(() => { expect(document.body.textContent).toContain('use its original request ID'); });
  expect(inputs(restored, 'MobileRemotePrompt')).toEqual([]);
  fill('Compare fixture counts');
  await vi.waitFor(() => { expect(button('Send follow-up').disabled).toBe(false); });
  answers['MobileRemoteControl'] = { agentThreadControl: { thread: THREAD, offer: null } };
  button('Send follow-up').click();
  await vi.waitFor(() => { expect(document.body.textContent).toContain('no longer offers this action'); });
  expect(inputs(restored, 'MobileRemotePrompt')).toEqual([]);
  expect(Object.values(localStorage)).toEqual([command]);

  answers['MobileRemoteControl'] = { agentThreadControl: { thread: THREAD, offer } };
  button('Check follow-up status').click();
  await vi.waitFor(() => { expect(button('Send follow-up').disabled).toBe(false); });
  button('Send follow-up').click();
  await vi.waitFor(() => { expect(inputs(restored, 'MobileRemotePrompt')).toEqual([first]); });
});
