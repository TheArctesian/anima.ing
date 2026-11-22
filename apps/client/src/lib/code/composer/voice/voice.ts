/** Where a dictation is (03 A.6.2). */
export type VoicePhase = 'preparing' | 'recording' | 'transcribing' | 'cleaning';

/** The elapsed time of a recording as "m:ss", with the limit after it when there is one: "0:42 / 2:00". */
export function clock(seconds: number, limit: number | null = null): string {
  const part = (value: number): string => `${String(Math.floor(value / 60))}:${String(Math.floor(value % 60)).padStart(2, '0')}`;
  return limit === null ? part(seconds) : `${part(seconds)} / ${part(limit)}`;
}

/** The words of a phase that has no clock. */
export function phaseWords(phase: VoicePhase): string {
  if (phase === 'transcribing') return 'Transcribing…';
  if (phase === 'cleaning') return 'Cleaning up…';
  return phase === 'recording' ? 'Recording' : 'Starting…';
}

/** The pill draws the last 24 levels of the microphone, the newest on the right (03 A.6.2, "Waveform"). */
export const BARS = 24;

/** The height of each bar as a share of the pill's 16 px: never under a fifth, so that silence still shows bars. */
export function bars(levels: readonly number[]): readonly number[] {
  const last = levels.slice(-BARS).map((level) => Math.max(0.2, Math.min(1, level)));
  return [...Array.from({ length: BARS - last.length }, () => 0.2), ...last];
}
