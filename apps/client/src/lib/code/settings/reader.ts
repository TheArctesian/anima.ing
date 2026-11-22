/**
 * The reader settings of the Code page (TC6; R-354, "All that apply"): the width of the chat, the contrast of quiet
 * text and lines, and the font size. They are kept on this device, like the typeface, and set on the document root, where
 * `lib/code/kit/code.css` turns them into the Code tokens.
 */

export const CHAT_WIDTHS = [
  { id: 'comfortable', label: 'Comfortable (default)' },
  { id: 'wide', label: 'Wide' },
  { id: 'full', label: 'Full' },
] as const;

export type ChatWidth = (typeof CHAT_WIDTHS)[number]['id'];

/**
 * The contrast in percent, as T3 Code's slider shows it, from 100 (the theme as drawn) up. T3 Code goes down to 50; less
 * than 100 would take quiet text under 4.5:1, so Anima starts at 100 (R-354).
 */
export const CONTRAST = { min: 100, max: 200, step: 5, default: 100 } as const;

/**
 * The size of chat text in pixels; the rest of the Code text follows in proportion. T3 Code goes from 12 to 20 px; under
 * 14 px the 13 px small text of Anima would go under 12 px, so Anima starts at 14 (R-354).
 */
export const FONT_SIZE = { min: 14, max: 20, default: 14 } as const;

export interface Reader {
  readonly width: ChatWidth;
  readonly contrast: number;
  readonly fontSize: number;
}

export const DEFAULT_READER: Reader = { width: 'comfortable', contrast: CONTRAST.default, fontSize: FONT_SIZE.default };

const KEYS = { width: 'anima.code.width', contrast: 'anima.code.contrast', fontSize: 'anima.code.font-size' } as const;

/** Each font size the setting offers, in pixels. */
export const FONT_SIZES: readonly number[] = Array.from({ length: FONT_SIZE.max - FONT_SIZE.min + 1 }, (_, index) => FONT_SIZE.min + index);

function read(storage: Storage, key: string): string | null {
  try {
    return storage.getItem(key);
  } catch {
    return null;
  }
}

/** A whole number inside the range and on its step, or `null` when the text is not one. */
function inRange(text: string | null, min: number, max: number, step: number): number | null {
  if (text === null || !/^\d+$/.test(text)) return null;
  const value = Number(text);
  return value >= min && value <= max && (value - min) % step === 0 ? value : null;
}

/** The settings saved on this device; each default where nothing valid is saved. */
export function storedReader(storage: Storage): Reader {
  const width = CHAT_WIDTHS.find((each) => each.id === read(storage, KEYS.width))?.id ?? DEFAULT_READER.width;
  const contrast = inRange(read(storage, KEYS.contrast), CONTRAST.min, CONTRAST.max, CONTRAST.step) ?? DEFAULT_READER.contrast;
  const fontSize = inRange(read(storage, KEYS.fontSize), FONT_SIZE.min, FONT_SIZE.max, 1) ?? DEFAULT_READER.fontSize;
  return { width, contrast, fontSize };
}

/** Save the settings on this device. A default is removed, not saved, so a later default applies. Throws when the storage refuses. */
export function storeReader(storage: Storage, reader: Reader): void {
  const values: Record<keyof Reader, string> = { width: reader.width, contrast: String(reader.contrast), fontSize: String(reader.fontSize) };
  for (const field of Object.keys(KEYS) as (keyof Reader)[]) {
    if (reader[field] === DEFAULT_READER[field]) storage.removeItem(KEYS[field]);
    else storage.setItem(KEYS[field], values[field]);
  }
}

/** The values `code.css` reads: the width as an attribute, the text scale and the contrast mix as properties. */
export function readerTokens(reader: Reader): { width: ChatWidth; textScale: string; contrastMix: string } {
  const mix = ((reader.contrast - CONTRAST.min) / (CONTRAST.max - CONTRAST.min)) * 100;
  return { width: reader.width, textScale: (reader.fontSize / FONT_SIZE.default).toFixed(4), contrastMix: `${String(Math.round(mix))}%` };
}

/** Mark the document root with the settings, so each part of the Code page follows without reading them. */
export function applyReader(root: HTMLElement, reader: Reader): void {
  const tokens = readerTokens(reader);
  root.dataset['codeWidth'] = tokens.width;
  root.style.setProperty('--code-text-scale', tokens.textScale);
  root.style.setProperty('--code-contrast', tokens.contrastMix);
}
