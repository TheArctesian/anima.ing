import type { TextRole, Typeface, TypefaceChoice } from './typeface';

const FACES: { readonly [R in TextRole]: Readonly<Record<Typeface<R>, readonly string[]>> } = {
  ui: {
    'ibm-plex-sans': ['200 1em "IBM Plex Sans"', '400 1em "IBM Plex Sans"', '500 1em "IBM Plex Sans"'],
    opendyslexic: ['400 1em "OpenDyslexic Nerd Font Propo"'],
  },
  mono: {
    'ibm-plex-mono': ['400 1em "IBM Plex Mono"'],
    opendyslexic: ['400 1em "OpenDyslexicM Nerd Font Mono"'],
  },
};

export const ALL_FIRST_PAINT_FACES: readonly string[] = [
  ...new Set([...Object.values(FACES.ui).flat(), ...Object.values(FACES.mono).flat()]),
];

/** A font that arrives after the first render moves the text (MOT-007). */
export function firstPaintFaces(choice: TypefaceChoice): readonly string[] {
  return [...new Set([...FACES.ui[choice.ui], ...FACES.mono[choice.mono]])];
}

/** The most a first render waits for its fonts: about the block period browsers use with `font-display: block` (R-042). */
export const FONT_WAIT_MS = 3000;

export async function waitForFirstPaintFonts(choice: TypefaceChoice, fonts: Pick<FontFaceSet, 'load'> | undefined): Promise<void> {
  if (fonts === undefined) return;
  const loaded = Promise.allSettled(firstPaintFaces(choice).map((face) => fonts.load(face)));
  let timer: ReturnType<typeof setTimeout> | undefined;
  const expired = new Promise<void>((resolve) => {
    timer = setTimeout(resolve, FONT_WAIT_MS);
  });
  try {
    await Promise.race([loaded, expired]);
  } finally {
    clearTimeout(timer);
  }
}
