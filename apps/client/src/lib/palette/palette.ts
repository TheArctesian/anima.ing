/** The colour palettes the operator can choose (UI-032). A third palette is one more entry here and its token block. */
export const PALETTES = [
  { id: 'graphite', label: 'Graphite' },
  { id: 'nord', label: 'Nord' },
] as const;

export type Palette = (typeof PALETTES)[number]['id'];

export const DEFAULT_PALETTE: Palette = 'graphite';

const STORAGE_KEY = 'anima.palette';

function isPalette(text: string | null): text is Palette {
  return PALETTES.some((palette) => palette.id === text);
}

export function storedPalette(storage: Storage): Palette {
  let text: string | null;
  try {
    text = storage.getItem(STORAGE_KEY);
  } catch {
    return DEFAULT_PALETTE;
  }
  return isPalette(text) ? text : DEFAULT_PALETTE;
}

/** Throws when the storage refuses (quota, privacy modes). */
export function storePalette(storage: Storage, palette: Palette): void {
  storage.setItem(STORAGE_KEY, palette);
}

/** The generated `tokens.css` gives each `data-palette` its colours (UI-032). */
export function applyPalette(root: HTMLElement, palette: Palette): void {
  root.dataset['palette'] = palette;
}
