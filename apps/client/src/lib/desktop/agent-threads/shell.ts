import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { rowsFrom, threadFrom, type ThreadsShell } from '$lib/agent-threads/thread';

/** The shell's error codes (THR-011). */
export type ThreadsShellProblem = 'caller' | 'home' | 'not_found' | 'failed';

const MESSAGES: Record<ThreadsShellProblem, string> = {
  caller: 'Only the main window can read the threads.',
  home: 'The home folder of this user is not known.',
  not_found: 'This thread is not on this computer any more.',
  failed: 'The threads could not be read.',
};

const PROBLEMS = Object.keys(MESSAGES);

/** The message is fixed text: nothing of the shell's error reaches the screen (LOG-007). */
export class ThreadsShellError extends Error {
  constructor(readonly problem: ThreadsShellProblem) {
    super(MESSAGES[problem]);
    this.name = 'ThreadsShellError';
  }
}

async function ask<T>(command: string, args: Record<string, unknown>, from: (answer: unknown) => T | null): Promise<T> {
  let answer: unknown;
  try {
    answer = await invoke(command, args);
  } catch (reason) {
    const code = typeof reason === 'object' && reason !== null && 'code' in reason ? reason.code : undefined;
    throw new ThreadsShellError(typeof code === 'string' && PROBLEMS.includes(code) ? (code as ThreadsShellProblem) : 'failed');
  }
  const value = from(answer);
  if (value === null) throw new ThreadsShellError('failed');
  return value;
}

/** The desktop shell's commands: the same library as `anima agents threads` (THR-011). */
export const desktopThreadsShell: ThreadsShell = {
  list: (program) => ask('agent_threads_list', { program, limit: 200 }, rowsFrom),
  read: (id) => ask('agent_thread_read', { id }, threadFrom),
};

/** The event contains only watch health, never a session path or text. */
export async function watchThreadFiles(changed: (status: 'ready' | 'changed' | 'unavailable') => void): Promise<() => void> {
  const stop = await listen<unknown>('agent-threads:changed', (event) => {
    if (event.payload === 'ready' || event.payload === 'changed' || event.payload === 'unavailable') changed(event.payload);
  });
  try {
    await invoke('agent_threads_watch');
    return stop;
  } catch (reason) {
    stop();
    throw reason;
  }
}
