import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { outcomeFrom, updateFrom, type Action, type Outcome, type Update } from '$lib/agent-threads/live/run';

/** The shell's codes (THR-128). */
export type RunnerProblem = 'caller' | 'not_running' | 'foreign' | 'bad_call' | 'notification' | 'older_daemon' | 'failed';

const MESSAGES: Readonly<Record<RunnerProblem, string>> = {
  caller: 'Only the main window can drive the threads.',
  not_running: 'The thread runner of this computer is not running. Start the Anima daemon.',
  foreign: 'The socket of the thread runner is not a private socket of this user.',
  bad_call: 'The call did not read.',
  notification: 'The notification was not shown.',
  older_daemon:
    'The daemon on this computer is older than this app and does not know this call. Quit Anima and open it again: the app updates the daemon at its start. Or install the daemon again in the settings of this computer, at Metrics source: Daemon.',
  failed: 'The thread runner did not answer.',
};

const PROBLEMS = Object.keys(MESSAGES);

/** The message is fixed text: nothing of the shell's error reaches the screen (LOG-007). */
export class RunnerShellError extends Error {
  constructor(readonly problem: RunnerProblem) {
    super(MESSAGES[problem]);
    this.name = 'RunnerShellError';
  }
}

function problemOf(reason: unknown): RunnerProblem {
  const code = typeof reason === 'object' && reason !== null && 'code' in reason ? reason.code : undefined;
  return typeof code === 'string' && PROBLEMS.includes(code) ? (code as RunnerProblem) : 'failed';
}

export interface RunnerShell {
  /** A refusal of the runner is an outcome, not an error. */
  call(action: Action): Promise<Outcome>;
  watch(onUpdate: (update: Update) => void, onLink: (linked: boolean) => void): Promise<() => void>;
  /** A system notification of this computer (THR-059). */
  notify(title: string, words: string): Promise<void>;
}

export const desktopRunnerShell: RunnerShell | null = isTauri()
  ? {
      async call(action) {
        let answer: unknown;
        try {
          answer = await invoke('agent_runner_call', { action });
        } catch (reason) {
          throw new RunnerShellError(problemOf(reason));
        }
        const outcome = outcomeFrom(answer);
        if (outcome === null) throw new RunnerShellError('failed');
        return outcome;
      },
      async watch(onUpdate, onLink) {
        const stops = await Promise.all([
          listen<unknown>('agent-runner:update', (event) => {
            const update = updateFrom(event.payload);
            if (update !== null) onUpdate(update);
          }),
          listen<unknown>('agent-runner:link', (event) => {
            onLink(event.payload === true);
          }),
        ]);
        try {
          await invoke('agent_runner_watch');
        } catch (reason) {
          for (const stop of stops) stop();
          throw new RunnerShellError(problemOf(reason));
        }
        return () => {
          for (const stop of stops) stop();
        };
      },
      async notify(title, words) {
        try {
          await invoke('agent_thread_notify', { title, words });
        } catch (reason) {
          throw new RunnerShellError(problemOf(reason));
        }
      },
    }
  : null;
