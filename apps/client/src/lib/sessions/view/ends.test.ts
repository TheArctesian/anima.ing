// @vitest-environment jsdom
import { flushSync } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { SessionEndReason } from '../ending';
import { installBlankCanvas } from '../tunnel/fake/canvas.fixture';
import { FakeTerminalSocket } from '../tunnel/terminal/fake/socket.fixture';
import { FakeResizeObserver, connectView, flags, installBrowserFakes, screenText, settle, showView } from './view.fixture.svelte';

let cleanups: (() => unknown)[] = [];

beforeEach(() => {
  vi.useFakeTimers();
  FakeTerminalSocket.reset();
  flags.visible = true;
  flags.focused = true;
  cleanups = [installBlankCanvas(), installBrowserFakes()];
});

afterEach(async () => {
  for (const cleanup of cleanups.reverse()) await cleanup();
  document.body.innerHTML = '';
  vi.useRealTimers();
});

function show() {
  const view = showView();
  cleanups.push(view.close);
  return view;
}

async function endedBy(end: SessionEndReason | undefined, code: number, recorded: SessionEndReason | null = null) {
  const view = show();
  view.answers.endReason = recorded;
  const socket = await connectView();
  if (end !== undefined) socket.end(end);
  socket.closeWith(code);
  await settle();
  return { ...view, text: view.target.querySelector('[data-reason]')?.textContent, hostKeyModal: document.querySelector('[role="alertdialog"]') };
}

describe('each end of a session (K4 §8.6, RA-007)', () => {
  it.each<[string, SessionEndReason | undefined, number, string]>([
    ['the shell exits (1000)', 'REMOTE_CLOSED', 1000, 'The device closed the session.'],
    ['a close with no end message (1000)', undefined, 1000, 'The device closed the session.'],
    ['the hub loses its SSH connection (1011)', 'GATEWAY_ERROR', 1011, 'The hub lost the connection to the device.'],
    ['the device refuses the credential (1011)', 'AUTH_FAILED', 1011, 'The device refused the sign-in, or no credential is stored.'],
    ['a protocol error (1002)', 'PROTOCOL_ERROR', 1002, 'The hub closed the session: the app sent something it does not accept.'],
    ['client silence (4002)', 'TIMEOUT', 4002, 'Timed out: the hub heard nothing from Anima.ing.'],
    ['a close of 4002 with no end message', undefined, 4002, 'Timed out: the hub heard nothing from Anima.ing.'],
    ['the hub shuts down (1001)', 'HUB_SHUTDOWN', 1001, 'The hub shut down.'],
  ])('RA-007: %s gives its ended panel', async (_name, end, code, expected) => {
    const { text, hostKeyModal } = await endedBy(end, code);
    expect(text).toBe(expected);
    expect(hostKeyModal).toBeNull();
  });

  it('SEC-007: a host key mismatch (end message and 4001) opens the host key warning, not a plain ended panel only', async () => {
    const { text, hostKeyModal } = await endedBy('HOST_KEY_MISMATCH', 4001);
    expect(text).toBe("The device's host key changed. Not connected.");
    expect(hostKeyModal?.textContent).toContain('host key');
  });

  it('SEC-007: the close code 4001 alone opens the host key warning', async () => {
    const { hostKeyModal } = await endedBy(undefined, 4001);
    expect(hostKeyModal).not.toBeNull();
  });

  it('SEC-007: a recorded HOST_KEY_MISMATCH opens the warning although the socket closed with another code', async () => {
    const { hostKeyModal } = await endedBy(undefined, 1000, 'HOST_KEY_MISMATCH');
    expect(hostKeyModal).not.toBeNull();
  });

  it('D-4: the hub row from endSession wins over the end message', async () => {
    const { text } = await endedBy('GATEWAY_ERROR', 1011, 'AUTH_FAILED');
    expect(text).toBe('The device refused the sign-in, or no credential is stored.');
  });

  it('RA-007: a refusal before the upgrade (503, seen as 1006) says the session could not be opened, with the hub reason', async () => {
    const view = showView();
    cleanups.push(view.close);
    view.answers.endReason = 'GATEWAY_ERROR';
    FakeResizeObserver.resize(800, 600);
    await settle();
    FakeTerminalSocket.latest().closeWith(1006);
    await settle();
    expect(view.target.querySelector('[data-reason]')?.textContent).toBe(
      'Could not open the session. The hub lost the connection to the device.',
    );
  });

  it('RA-007: an end message with a reason this client does not know is ignored; the close code decides', async () => {
    const view = showView();
    cleanups.push(view.close);
    const socket = await connectView();
    socket.end('SOMETHING_NEWER');
    socket.closeWith(1011);
    await settle();
    expect(view.target.querySelector('[data-reason]')?.textContent).toBe('The hub lost the connection to the device.');
  });
});

describe('ended panel and Reconnect (RA-007)', () => {
  it('RA-007: the panel shows the recorded reason, which wins over the end message and the close code (D-4)', async () => {
    const { target, answers, calls, ended } = show();
    const socket = await connectView();
    answers.endReason = 'REMOTE_CLOSED';
    socket.end('GATEWAY_ERROR');
    socket.closeWith(1011);
    await settle();
    expect(calls.endSession).toEqual(['session-1']);
    expect(target.querySelector('[data-reason]')?.textContent).toBe('The device closed the session.');
    expect(target.querySelector('.ended')?.classList.contains('under-steps')).toBe(false);
    expect(ended.map((ending) => ending.reason)).toEqual(['REMOTE_CLOSED']);
  });

  it('RA-007: Reconnect calls startSession once, opens a new socket with a new token and clears the old screen', async () => {
    const { target, calls, heard } = show();
    const first = await connectView();
    first.output('old session text');
    first.closeWith(1000);
    await settle();
    expect(screenText(target)).toContain('old session text');
    const reconnect = [...target.querySelectorAll('button')].find((button) => button.textContent.includes('Reconnect'));
    reconnect?.click();
    reconnect?.click();
    await settle();
    expect(calls.startSession).toHaveLength(2);
    expect(FakeTerminalSocket.made).toHaveLength(2);
    const [old, fresh] = FakeTerminalSocket.made.map((socket) => new URL(socket.url).searchParams.get('token'));
    expect(fresh).not.toBe(old);
    FakeTerminalSocket.latest().open();
    flushSync();
    expect(screenText(target)).not.toContain('old session text');
    expect(heard.titles.at(-1)).toBe('');
  });

  it('a refused start shows fixed text, not the hub message, and offers Reconnect under the steps, whose status says where they stopped', async () => {
    const { target } = show();
    FakeResizeObserver.resize(800, 600);
    await settle();
    FakeTerminalSocket.latest().closeWith(1006);
    await settle();
    expect(target.querySelector('[data-reason]')?.textContent).toBe('Could not open the session.');
    expect([...target.querySelectorAll('button')].some((button) => button.textContent.includes('Reconnect'))).toBe(true);
    const status = target.querySelector('[data-caption]');
    expect([status?.getAttribute('role'), status?.textContent]).toEqual(['status', expect.stringContaining('Stopped at')]);
    expect(target.querySelector('.ended')?.classList.contains('under-steps')).toBe(true);
  });
});
