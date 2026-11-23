/** Undrawn output at which skipping starts, and the level at which drawing resumes, in bytes. */
export const HIGH_WATER_BYTES = 16 * 1024 * 1024;
export const LOW_WATER_BYTES = 4 * 1024 * 1024;

export type Admission = 'draw' | 'skip' | 'skip-and-tell';

/** xterm.js throws and loses output once 50 MB wait, so above `high` output is skipped until the backlog is `low` (K4 §8.5). */
export function outputFlow(high = HIGH_WATER_BYTES, low = LOW_WATER_BYTES) {
  let pending = 0;
  let skipping = false;
  return {
    /** On `draw`, call `drawn(size)` when xterm.js has taken the bytes. */
    admit: (size: number): Admission => {
      if (skipping && pending <= low) skipping = false;
      if (!skipping && pending + size > high) {
        skipping = true;
        return 'skip-and-tell';
      }
      if (skipping) return 'skip';
      pending += size;
      return 'draw';
    },
    drawn: (size: number) => {
      pending = Math.max(0, pending - size);
    },
  };
}
