/**
 * The skin of Anima on the tree library of T3 Code (`@pierre/trees`). The library draws inside a shadow root and reads
 * its look from CSS variables; the custom properties of the page pass into a shadow root, so each value here is a token
 * of Anima. The rows keep the sizes of T3 Code: 12px text, a radius of 6px, hover at 7% and selection at 12% of the
 * text colour, and a clipped name with an ellipsis (`pierre-tree-theme.ts`).
 */
export const TREE_SKIN = `
:host {
  --trees-bg-override: transparent;
  --trees-fg-override: var(--text);
  --trees-fg-muted-override: var(--text-secondary);
  --trees-accent-override: var(--focus-ring);
  --trees-focus-ring-color-override: var(--focus-ring);
  --trees-selected-bg-override: var(--surface-selected);
  --trees-selected-fg-override: var(--text);
  --trees-hover-bg-override: var(--surface-hover);
  --trees-border-color-override: var(--border-subtle);
  --trees-indent-guide-bg-override: var(--border-subtle);
  --trees-font-family-override: var(--font-ui);
  --trees-font-size-override: var(--text-xs);
  --trees-border-radius-override: var(--radius-md);
  --trees-status-added-override: var(--status-up-text);
  --trees-status-untracked-override: var(--status-up-text);
  --trees-status-deleted-override: var(--status-down-text);
  --trees-status-modified-override: var(--status-warning-text);
  --trees-status-renamed-override: var(--status-warning-text);
  --trees-status-ignored-override: var(--text-secondary);
  --trees-input-bg-override: var(--surface-sunken);
  --trees-search-bg-override: var(--surface-sunken);
  --trees-search-fg-override: var(--text);
  --trees-scrollbar-thumb-override: var(--border-strong);
}
[data-truncate-content='visible'] { overflow: hidden; text-overflow: ellipsis; }
[data-truncate-marker] { display: none; }
button[data-type='item'], button[data-type='item']::before { border-radius: var(--radius-md); }
`;
