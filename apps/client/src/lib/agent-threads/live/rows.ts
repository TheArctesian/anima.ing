/** The threads that the runner drives beside the threads read from session files: one list, one row each (THR-056). Pure. */

import type { Tail, ThreadRow } from '../thread';
import type { RunState } from './run';
import type { LiveThread } from './timeline';

const TAILS: Readonly<Record<RunState, Tail>> = {
  needs_you: 'asks',
  starting: 'starting',
  working: 'running',
  idle: 'turn_ended',
  stopped: 'stopped',
  unknown: 'nothing',
};

/** A driven thread as a row of the list: its state is what its program reports, not a reading of a file. */
export function liveRow(thread: LiveThread): ThreadRow {
  const row = thread.row;
  return {
    id: row.id,
    program: row.program,
    title: row.title === '' ? null : row.title,
    folder: row.folder,
    branch: null,
    model: row.model,
    started: row.started,
    last: thread.last,
    tail: thread.asks.length > 0 ? 'asks' : TAILS[row.state],
  };
}

/** Driven threads first; a driven thread's own session file is left out, as Claude Code takes the thread's ID as its session. */
export function mergedRows(files: readonly ThreadRow[], live: readonly LiveThread[]): readonly ThreadRow[] {
  const driven = new Set(live.flatMap((thread) => [thread.row.id, ...(thread.row.session === null ? [] : [thread.row.session])]));
  return [...live.map(liveRow), ...files.filter((row) => !driven.has(row.id))];
}
