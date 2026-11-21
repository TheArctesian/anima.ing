import type { CodeIconName } from '../../icons/icons';
import type { Can } from '../draft';

/** What a follow-up does while a turn runs: it goes into the turn, or waits in the queue (03 B.7.3). */
export type FollowUp = 'steer' | 'queue';

/** What the round button does when it is pressed. */
export type Action = 'send' | 'steer' | 'queue' | 'update';

/** The facts the primary actions are worked out from. */
export interface Turn {
  /** A turn runs and can take a follow-up or be stopped. */
  readonly running: boolean;
  /** The prompt has words or attachments. */
  readonly filled: boolean;
  /** A send is on its way. */
  readonly busy: boolean;
  /** A queued message is open in the composer. */
  readonly editing: boolean;
  /** The person's default for a follow-up; T3's default is to steer. */
  readonly followUp: FollowUp;
  /** ⌘ or Ctrl is held: the other follow-up. */
  readonly alternate: boolean;
  readonly steer: Can;
  readonly queue: Can;
  /** Why nothing can be sent at all, when that is so: the program ended. */
  readonly blocked: string | null;
}

export interface SendFace {
  readonly action: Action;
  readonly icon: CodeIconName | 'arrow';
  /** The accessible name (03 B.7.1, the icon table). */
  readonly label: string;
  /** The tip: the reason when it is disabled, the two follow-ups while a turn runs, else the name. */
  readonly tip: string;
  readonly disabled: boolean;
}

/** The follow-up a press does now: the default, the other one with the modifier, and never one the engine lacks. */
export function followUpOf(turn: Pick<Turn, 'followUp' | 'alternate' | 'steer' | 'queue'>): FollowUp {
  const other: FollowUp = turn.followUp === 'steer' ? 'queue' : 'steer';
  const asked = turn.alternate ? other : turn.followUp;
  const fallback: FollowUp = asked === 'steer' ? 'queue' : 'steer';
  return turn[asked].can || !turn[fallback].can ? asked : fallback;
}

function followUpTip(turn: Turn, modifier: string): string {
  const first = followUpOf({ ...turn, alternate: false });
  const second: FollowUp = first === 'steer' ? 'queue' : 'steer';
  const lacking = turn[second];
  // With one follow-up only, the tip says why the other is not there.
  if (!lacking.can) return `Click to ${first}. ${lacking.reason}`;
  return `Click to ${first}, ${modifier}-click to ${second}`;
}

const FACES: Readonly<Record<Action, Pick<SendFace, 'icon' | 'label'>>> = {
  send: { icon: 'arrow', label: 'Submit message' },
  steer: { icon: 'corner-up-right', label: 'Steer message' },
  queue: { icon: 'list-plus', label: 'Queue message' },
  update: { icon: 'check', label: 'Update queued message' },
};

/** The round button (03 B.7.1): its action, icon, name, tip and whether it can be pressed. */
export function sendFace(turn: Turn, modifier: string): SendFace {
  const action: Action = turn.editing ? 'update' : turn.running ? followUpOf(turn) : 'send';
  const face = FACES[action];
  if (turn.blocked !== null) return { action, ...face, tip: turn.blocked, disabled: true };
  if (turn.busy) {
    const words = turn.editing ? 'Updating queued message' : 'Submitting message';
    return { action, ...face, label: words, tip: words, disabled: true };
  }
  const tip = turn.running && !turn.editing ? followUpTip(turn, modifier) : face.label;
  return { action, ...face, tip, disabled: !turn.filled };
}

/**
 * Stop stands in place of send while a turn runs and the prompt is empty, and beside it while a stop is on its way
 * (03 B.7.2).
 */
export function showsStop(turn: Pick<Turn, 'running' | 'filled' | 'editing'>, stop: StopPhase): boolean {
  if (stop === 'pending' || stop === 'timed_out') return true;
  return turn.running && !turn.filled && !turn.editing;
}

export function showsSend(turn: Pick<Turn, 'running' | 'filled' | 'editing'>): boolean {
  return !turn.running || turn.filled || turn.editing;
}

/**
 * A stop that was asked for: `pending` for at most ten seconds, then `timed_out` and it can be pressed again;
 * `landed` for half a second after the turn stopped, so that a second press of Stop does not send (03 B.7.2).
 */
export type StopPhase = 'none' | 'pending' | 'timed_out' | 'landed';

export const STOP_PENDING_MS = 10_000;
export const STOP_LANDED_MS = 500;

/** The phase after the turn's running state changed. */
export function stopAfter(phase: StopPhase, running: boolean): StopPhase {
  if ((phase === 'pending' || phase === 'timed_out') && !running) return 'landed';
  if (phase === 'landed' && running) return 'none';
  return phase;
}

/** What Enter does (03 B.12.1, the default setting): it sends; with Shift or Alt, or while an input method composes, it types. */
export function enterSends(press: { readonly key: string; readonly shift: boolean; readonly alt: boolean; readonly composing: boolean }): boolean {
  return press.key === 'Enter' && !press.shift && !press.alt && !press.composing;
}

/** "⌘" on a Mac, "Ctrl" elsewhere: the modifier the tips name. */
export function modifierWord(platform: string): string {
  return /mac|iphone|ipad/i.test(platform) ? '⌘' : 'Ctrl';
}
