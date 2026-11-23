import { invoke } from '@tauri-apps/api/core';

/** The shell's codes (LOG-008). */
export type LogFolderProblem = 'unavailable' | 'failed';

const MESSAGES: Record<LogFolderProblem, string> = {
  unavailable: 'The log folder is not available.',
  failed: 'The log folder could not be opened.',
};

/** The message is fixed text; nothing from the shell's error is kept. */
export class LogFolderError extends Error {
  constructor(readonly problem: LogFolderProblem) {
    super(MESSAGES[problem]);
    this.name = 'LogFolderError';
  }
}

function problemOf(reason: unknown): LogFolderProblem {
  const code = typeof reason === 'object' && reason !== null && 'code' in reason ? reason.code : undefined;
  return code === 'unavailable' ? 'unavailable' : 'failed';
}

export async function openLogFolder(): Promise<void> {
  try {
    await invoke('process_log_folder_open');
  } catch (reason) {
    throw new LogFolderError(problemOf(reason));
  }
}

/** Callers pass fixed text only: the plugin writes whatever it is given to a file on disk (LOG-007). */
export function warnToProcessLog(text: string): void {
  void import('@tauri-apps/plugin-log')
    .then(({ warn }) => warn(text))
    .catch((reason: unknown) => {
      console.warn('The process log did not take a line:', reason instanceof Error ? reason.name : 'unknown');
    });
}
