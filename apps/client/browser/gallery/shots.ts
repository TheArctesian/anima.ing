import type { Locator, Page } from '@playwright/test';
import type { Fixtures } from '../synthetic/fixtures';

// One OpenDyslexic pass besides the themes (UI-019).

export type ThemeName = 'dark' | 'light';

export interface Mode {
  readonly theme: ThemeName;
  readonly width: number;
  readonly dyslexic: boolean;
  readonly folder: string;
}

/** `GALLERY_WIDTHS=800,1440` shoots other widths than the three of the gallery. */
const asked = (process.env['GALLERY_WIDTHS'] ?? '').split(',').map(Number).filter((width) => Number.isInteger(width) && width >= 320);
export const WIDTHS: readonly number[] = asked.length > 0 ? asked : [800, 1280, 1600];
export const HEIGHT = 900;
const THEMES: readonly ThemeName[] = ['dark', 'light'];
const DYSLEXIC_WIDTH = WIDTHS.includes(1280) ? 1280 : (WIDTHS[0] ?? 1280);

function mode(theme: ThemeName, width: number, dyslexic: boolean): Mode {
  return { theme, width, dyslexic, folder: `${theme}-${String(width)}${dyslexic ? '-dyslexic' : ''}` };
}

/** Dark first, as the app's default. */
export const MODES: readonly Mode[] = [
  ...THEMES.flatMap((theme) => WIDTHS.map((width) => mode(theme, width, false))),
  mode('dark', DYSLEXIC_WIDTH, true),
];

export function storedSettings(chosen: Mode): Record<string, string> {
  const typeface = chosen.dyslexic ? 'opendyslexic' : 'fira-code';
  const palette = process.env['GALLERY_PALETTE'];
  return {
    'anima.theme': chosen.theme,
    'anima.typeface.ui': typeface,
    'anima.typeface.mono': typeface,
    ...(palette === undefined ? {} : { 'anima.palette': palette }),
  };
}

export interface Screen {
  readonly name: string;
  readonly signedOut?: true;
  readonly fixtures?: (loaded: Fixtures) => Fixtures;
  readonly open: (page: Page) => Promise<void>;
  readonly target?: (page: Page) => Locator;
  /** The hub holds an answer on purpose, so the page never goes network-quiet: shoot without waiting for it. */
  readonly loading?: true;
  readonly windowOnly?: true;
}
