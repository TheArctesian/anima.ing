import { describe, expect, it } from 'vitest';
import { MOST_PART_BYTES } from '$lib/agent-threads/live/attachments';
import { LiveThreads } from '$lib/agent-threads/live/live.svelte';
import type { Action, Outcome, RunRow } from '$lib/agent-threads/live/run';
import { madeShell, THREAD } from '$lib/agent-threads/live/tests/frames.fixture';
import { runEngine } from '../engine';
import { shownName } from './shown-name';
import { base64, upload, type AttachCall } from './upload';

const AWAY = { front: false, chosen: null };
const row: RunRow = { id: THREAD, program: 'claude_code', folder: '/w/anima.ing', mode: 'full_access', model: null, state: 'idle', session: null, title: 'Fix the test', started: new Date(0), ended: null, open: [] };

/** A runner that takes each part and counts its bytes, as the real one answers `attached`. */
function runner(refuse: (action: Action) => Outcome | null = () => null) {
  let received = 0;
  const shell = madeShell((action) => {
    const refusal = refuse(action);
    if (refusal !== null) return refusal;
    if (action.call !== 'attach' && action.call !== 'attach_more') return { outcome: 'ok' };
    received = (action.call === 'attach' ? 0 : received) + atob(action.bytes).length;
    return { outcome: 'attached', id: 'f1', received };
  });
  const live = new LiveThreads(shell, () => AWAY, () => new Date(0));
  // The call of the page, as `ChatFoot` gives it to the composer.
  const onAttach: AttachCall = (act) => live.call({ ...act, thread: THREAD });
  const onSend = (text: string, files?: readonly string[]) => live.call({ call: 'send', thread: THREAD, text, ...(files === undefined ? {} : { attachments: files }) });
  return { shell, onAttach, onSend };
}

const watch = { told: () => undefined, gone: () => false };

function fileOf(size: number, name = 'shot.png'): File {
  return new File([new Uint8Array(size).fill(7)], name, { type: 'image/png' });
}

describe('a file on its way to the runner', () => {
  it('TH111: goes as attach and attach_more in parts of 2 MiB, in the frames of the runner', async () => {
    const { shell, onAttach } = runner();
    const told: number[] = [];
    const size = 2 * MOST_PART_BYTES + 5;
    const went = await upload(onAttach, fileOf(size), 'image/png', { told: (sent) => told.push(sent), gone: () => false });
    expect(went).toEqual({ state: 'sent', id: 'f1' });
    const whole = base64(new Uint8Array(MOST_PART_BYTES).fill(7));
    expect(shell.calls).toEqual([
      { call: 'attach', thread: THREAD, name: 'shot.png', mime: 'image/png', size, bytes: whole },
      { call: 'attach_more', thread: THREAD, attachment: 'f1', bytes: whole },
      { call: 'attach_more', thread: THREAD, attachment: 'f1', bytes: base64(new Uint8Array(5).fill(7)) },
    ]);
    expect(told).toEqual([MOST_PART_BYTES, 2 * MOST_PART_BYTES, size]);
  });

  it('TH111: a refusal of the runner gives its words, and the file that it began is given up with unattach', async () => {
    const said = 'shot.png is too large: the limit is 10 MiB.';
    const { shell, onAttach } = runner((action) => (action.call === 'attach_more' ? { outcome: 'refused', why: 'too_large', said } : null));
    expect(await upload(onAttach, fileOf(MOST_PART_BYTES + 1), 'image/png', watch)).toEqual({ state: 'failed', words: said });
    expect(shell.calls.at(-1)).toEqual({ call: 'unattach', thread: THREAD, attachment: 'f1' });
  });

  it('stops between two parts when the person removed it, and says so to the runner', async () => {
    const { shell, onAttach } = runner();
    let parts = 0;
    const went = await upload(onAttach, fileOf(3 * MOST_PART_BYTES), 'image/png', { told: () => (parts += 1), gone: () => parts >= 1 });
    expect(went).toEqual({ state: 'dropped' });
    expect(shell.calls.map((action) => action.call)).toEqual(['attach', 'unattach']);
  });

  it('fails with words when the runner counts different bytes, when it does not answer and when the file cannot be read', async () => {
    const short = runner((action) => (action.call === 'attach' ? { outcome: 'attached', id: 'f1', received: 3 } : null));
    expect(await upload(short.onAttach, fileOf(10), 'image/png', watch)).toMatchObject({ state: 'failed', words: expect.stringContaining('did not take the file whole') });
    const other = runner((action) => (action.call === 'attach_more' ? { outcome: 'attached', id: 'f2', received: MOST_PART_BYTES + 1 } : null));
    expect(await upload(other.onAttach, fileOf(MOST_PART_BYTES + 1), 'image/png', watch)).toMatchObject({ state: 'failed' });
    expect(other.shell.calls.at(-1)).toEqual({ call: 'unattach', thread: THREAD, attachment: 'f1' });
    const silent: AttachCall = () => Promise.reject(new Error('closed'));
    expect(await upload(silent, fileOf(10), 'image/png', watch)).toEqual({ state: 'failed', words: 'The thread runner did not answer.' });
    const folder = { name: 'Pictures', size: 64, slice: () => ({ arrayBuffer: () => Promise.reject(new Error('a folder')) }) };
    expect(await upload(runner().onAttach, folder, 'application/octet-stream', watch)).toEqual({ state: 'failed', words: "'Pictures' is empty or could not be read." });
  });
});

describe('the files of a chat that the runner drives', () => {
  it('TH111: a send names the ids of its files, and a removed file is given up with unattach', async () => {
    const { shell, onAttach, onSend } = runner();
    const engine = runEngine(row, { onSend, onInterrupt: () => Promise.resolve({ ok: true, outcome: { outcome: 'ok' } }), onAttach }, null);
    expect(engine.attach.can).toBe(true);
    const went = await engine.upload?.(fileOf(4), 'image/png', watch);
    expect(went).toEqual({ state: 'sent', id: 'f1' });
    const attachment = { id: 'a1', name: 'shot.png', kind: 'image' as const, size: 4, mime: 'image/png', preview: null, state: { state: 'ready' as const }, remote: 'f1' };
    await engine.send({ prompt: 'Look at this', attachments: [attachment] });
    engine.discard?.('f2');
    expect(shell.calls.slice(1)).toEqual([
      { call: 'send', thread: THREAD, text: 'Look at this', attachments: ['f1'] },
      { call: 'unattach', thread: THREAD, attachment: 'f2' },
    ]);
    await engine.send({ prompt: 'No files', attachments: [] });
    expect(shell.calls.at(-1)).toEqual({ call: 'send', thread: THREAD, text: 'No files' });
  });

  it('keeps a message with files out of the queue, and sends it with them when no turn runs', async () => {
    const { shell, onAttach, onSend } = runner();
    const calls = { onSend, onInterrupt: () => Promise.resolve({ ok: true as const, outcome: { outcome: 'ok' as const } }), onAttach };
    const message = { prompt: 'later', attachments: [{ id: 'a1', name: 'n.txt', kind: 'text' as const, size: 1, mime: 'text/plain', preview: null, state: { state: 'ready' as const }, remote: 'f9' }] };
    const refused = await runEngine({ ...row, state: 'working' }, calls, null).enqueue(message);
    expect(refused).toMatchObject({ ok: false, words: expect.stringContaining('takes no files yet') });
    expect(shell.calls).toEqual([]);
    await runEngine(row, calls, null).enqueue(message);
    expect(shell.calls).toEqual([{ call: 'send', thread: THREAD, text: 'later', attachments: ['f9'] }]);
  });
});

describe('the name of a file as the program is told it', () => {
  it('TH111: is made by the rule of the runner', () => {
    expect(shownName('../../etc/passwd')).toBe('passwd');
    expect(shownName('..\\x\\.env')).toBe('env');
    expect(shownName('a\u0000b\nc.png')).toBe('abc.png');
    expect(shownName('gpj.\u202eexe.png')).toBe('gpj.exe.png');
    expect(shownName('a.txt\u2028- b.txt')).toBe('a.txt- b.txt');
    expect(shownName('..')).toBe('attachment');
    expect(shownName('/')).toBe('attachment');
    expect(new TextEncoder().encode(shownName('é'.repeat(200))).length).toBeLessThanOrEqual(120);
    expect(shownName('<img src=x onerror=1>.png')).toBe('_img src_x onerror_1_.png');
    expect(shownName('résumé 2026.pdf')).toBe('résumé 2026.pdf');
  });
});

describe('what the runner is told of a file', () => {
  it('TH111: the name in the attach frame is the shown one, of at most 120 bytes, whatever the name of the file is', async () => {
    const { shell, onAttach } = runner();
    const long = `../${'n'.repeat(5000)}${String.fromCodePoint(0x202e)}gpj.exe`;
    await upload(onAttach, fileOf(3, long), 'image/png', watch);
    const first = shell.calls[0];
    if (first?.call !== 'attach') throw new Error('no attach frame');
    expect(first.name).toBe('n'.repeat(120));
    expect(first.name).toBe(shownName(long));
    // The runner makes its name of this one by the same rule: the two are the same.
    expect(shownName(first.name)).toBe(first.name);
  });

  it('TH111: gives the words of the runner when too many files wait there, of all chats together', async () => {
    const said = 'shot.png is too large: too many files wait to go; send or drop some first.';
    const { shell, onAttach } = runner((action) => (action.call === 'attach' ? { outcome: 'refused', why: 'too_large', said } : null));
    expect(await upload(onAttach, fileOf(3), 'image/png', watch)).toEqual({ state: 'failed', words: said });
    expect(shell.calls.map((action) => action.call)).toEqual(['attach']);
  });
});
