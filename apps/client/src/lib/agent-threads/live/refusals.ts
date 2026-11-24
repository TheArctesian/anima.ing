import type { Refusal } from './run';

/** The soft-limit sentence (THR-114), word for word the same as `anima agents start` and the runner. */
export const OVER_LIMIT = 'You are over the recommended guidance and at risk of cyberpsychosis from running this many sessions. Do you want to raise the limit?';

/** A refusal of the runner in words for the person; the same words as `anima agents`. */
export const REFUSAL_WORDS: Readonly<Record<Refusal, string>> = {
  unknown_thread: 'No thread of this computer has that ID.',
  folder_not_permitted: 'That folder is not an absolute folder of this computer.',
  program_not_found: 'That program is not on the PATH of your login, or that path is not a copy the runner finds.',
  program_does_not_answer: 'The program did not answer.',
  bad_value: "A value is not permitted (a model name takes letters, digits, '.', '_', '-', '[' and ']').",
  request_gone: 'That request or queued prompt is gone: it was answered, closed or sent, or its program ended.',
  answer_does_not_fit: 'That answer does not fit the request.',
  not_running: 'The program of that thread is not running.',
  holder: 'The holder of the programs did not take the command.',
  from_an_agent: 'Only the person can answer a request, raise the limit, start a thread, change the list of threads, list the files of a thread, change the queue of a thread or change its model, effort or mode: this call came from a process of an agent.',
  running: 'The program of that thread still runs: stop it first, then remove it from the list.',
  over_limit: OVER_LIMIT,
  queue_full: 'That thread has 20 queued prompts: send or drop one first.',
  cannot_change: 'The program of that thread cannot change that after its start.',
  too_large: 'That file is too large to send with the prompt.',
  cannot_attach: 'That file cannot go with the prompt.',
};
