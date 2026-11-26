import { beforeEach, describe, expect, it, vi } from 'vitest';
import { BROWSER, MOBILE, MOBILE_APPLE } from '$lib/platform/answers.fixture';
import type { Capabilities } from '$lib/platform/capabilities';
import { playHaptic } from './haptics';

const shell = vi.hoisted(() => ({ invoke: vi.fn(), answers: undefined as Capabilities | undefined }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: shell.invoke }));
vi.mock('$lib/platform/platform', () => ({ capabilities: () => shell.answers }));

beforeEach(() => {
  shell.invoke.mockReset();
  shell.invoke.mockResolvedValue(null);
});

describe('playHaptic (mobile plan M4-4)', () => {
  it('MOT-050: the iPhone app asks the shell to play the pattern', () => {
    shell.answers = MOBILE_APPLE;
    playHaptic('selection');
    playHaptic('error');
    expect(shell.invoke.mock.calls).toEqual([['haptic_play', { feel: 'selection' }], ['haptic_play', { feel: 'error' }]]);
  });

  it('MOT-050: the Android app and a browser play nothing', () => {
    for (const answers of [MOBILE, BROWSER]) {
      shell.answers = answers;
      playHaptic('success');
    }
    expect(shell.invoke).not.toHaveBeenCalled();
  });

  it('MOT-050: a shell that does not play it throws nothing to the caller and says so in the console', async () => {
    shell.answers = MOBILE_APPLE;
    shell.invoke.mockRejectedValue(new Error('iOS did not play the haptic feedback.'));
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    expect(() => { playHaptic('success'); }).not.toThrow();
    await vi.waitFor(() => { expect(warn).toHaveBeenCalledWith('The haptic feedback did not play:', 'Error'); });
    warn.mockRestore();
  });
});
