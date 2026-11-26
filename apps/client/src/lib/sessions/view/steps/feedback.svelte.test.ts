// @vitest-environment jsdom
import { flushSync } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { playStartFeedback } from './feedback.svelte';
import type { Progress } from './steps';

const played = vi.hoisted(() => [] as string[]);
vi.mock('$lib/desktop/haptics/haptics', () => ({ playHaptic: (feel: string) => { played.push(feel); } }));

const STARTING: Progress = { ticketed: false, waking: false, opened: false, connected: false, stopped: false };
const OPENED: Progress = { ...STARTING, ticketed: true, opened: true };
const CONNECTED: Progress = { ...OPENED, connected: true };

/** The feedback that a session whose steps go through `steps` plays, one change at a time. */
function feltOver(steps: readonly Progress[]): string[] {
  const shown = $state({ progress: STARTING });
  const stop = $effect.root(() => {
    playStartFeedback(() => shown.progress);
  });
  flushSync();
  for (const step of steps) {
    shown.progress = step;
    flushSync();
  }
  stop();
  return [...played];
}

beforeEach(() => {
  played.length = 0;
});

describe('playStartFeedback (MOT-050, mobile plan M4-4)', () => {
  it('MOT-050: a start that reaches connected plays one success, also when its tunnel changes again', () => {
    expect(feltOver([OPENED, CONNECTED, { ...CONNECTED, waking: true }])).toEqual(['success']);
  });

  it('MOT-050: a start that stops before it connected plays one error, while it ends and once it ended', () => {
    const stopped = { ...OPENED, stopped: true };
    expect(feltOver([OPENED, stopped, { ...stopped }])).toEqual(['error']);
  });

  it('MOT-050: a hub that refuses the ticket is an error too', () => {
    expect(feltOver([{ ...STARTING, stopped: true }])).toEqual(['error']);
  });

  it('MOT-050: a session that ends after it connected plays nothing for its end', () => {
    expect(feltOver([CONNECTED, { ...CONNECTED, stopped: true }])).toEqual(['success']);
  });

  it('MOT-050: Reconnect after a failed start plays the outcome of the new start', () => {
    const stopped = { ...OPENED, stopped: true };
    expect(feltOver([stopped, STARTING, OPENED, CONNECTED])).toEqual(['error', 'success']);
  });

  it('MOT-050: a view that opens on a session that already connected plays nothing for it', () => {
    const shown = $state({ progress: CONNECTED });
    const stop = $effect.root(() => {
      playStartFeedback(() => shown.progress);
    });
    flushSync();
    stop();
    expect(played).toEqual([]);
  });
});
