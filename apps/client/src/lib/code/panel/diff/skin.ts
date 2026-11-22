/**
 * The skin of Anima on the diff library of T3 Code (`@pierre/diffs`). The library draws inside a shadow root and reads
 * its look from CSS variables; the custom properties of the page pass into a shadow root, so each value here is a token
 * of Anima. For an added or a removed line the library mixes the colour given here into the surface (12% in the
 * light, 20% in the dark), so the value is the status colour itself and not a tint of it. The list of variables is the one that T3 Code overrides (`lib/diffRendering.ts`).
 */
export const DIFF_SKIN = `
:host,
[data-diffs-header],
[data-diff],
[data-file],
[data-error-wrapper],
[data-virtualizer-buffer] {
  --diffs-header-font-family: var(--font-ui) !important;
  --diffs-font-family: var(--font-mono) !important;
  --diffs-font-size: var(--text-sm) !important;
  --diffs-bg: var(--surface-base) !important;
  --diffs-light-bg: var(--surface-base) !important;
  --diffs-dark-bg: var(--surface-base) !important;
  --diffs-token-light-bg: transparent;
  --diffs-token-dark-bg: transparent;
  --diffs-fg-number-override: var(--text-secondary);
  --diffs-bg-context-override: var(--surface-base);
  --diffs-bg-hover-override: var(--surface-hover);
  --diffs-bg-separator-override: var(--surface-sunken);
  --diffs-bg-buffer-override: var(--surface-sunken);
  --diffs-addition-color-override: var(--status-up-text);
  --diffs-deletion-color-override: var(--status-down-text);
  --diffs-bg-addition-override: var(--status-up-text);
  --diffs-bg-addition-number-override: var(--status-up-text);
  --diffs-bg-deletion-override: var(--status-down-text);
  --diffs-bg-deletion-number-override: var(--status-down-text);
  --diffs-bg-selection-override: var(--surface-selected);
  background-color: var(--surface-base) !important;
}
`;

/** The syntax themes: Nord, the palette of Anima, in the dark; a plain light theme in the light. */
export const DIFF_THEMES = { dark: 'nord', light: 'github-light' } as const;
