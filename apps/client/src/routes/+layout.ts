import { waitForFirstPaintFonts } from '$lib/typeface/first-paint';
import { storedTypefaces } from '$lib/typeface/typeface';

export const prerender = false;
export const ssr = false;

/** The first render waits for the chosen fonts, so no later swap moves the page (MOT-007). */
export async function load(): Promise<void> {
  await waitForFirstPaintFonts(storedTypefaces(localStorage), 'fonts' in document ? document.fonts : undefined);
}
