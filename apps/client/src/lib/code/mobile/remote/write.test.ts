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

function open(answers: Record<string, unknown>, search = '') {
  url = new URL(`https://hub.test/code${search}`);
  const mock = fakeHub(answers);
  hub = mock.hub;
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

afterEach(async () => {
  if (app !== undefined) await unmount(app);
  app = undefined;
  vi.restoreAllMocks();
});

it('retries the same start and only offers to open the thread after the hub binds it', async () => {
  const answers: Record<string, unknown> = {
    MobileThreadInbox: { agentThreads: { nodes: [], pageInfo: { hasNextPage: false } } },
    MobileRemoteTargets: { agentThreadStartTargets: [TARGET] },
    MobileRemoteStart: { code: 'unreachable', message: 'The response was lost.' },
  };
  const requests = open(answers);
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
