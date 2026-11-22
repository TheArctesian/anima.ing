import { error } from '@sveltejs/kit';
import { dev } from '$app/environment';

/**
 * The details card and the right panel of the Code section in each state (TC5, stage 5 of the T3 Code copy) exists in development, and in a build made with
 * `VITE_ANIMA_SPECIMEN=1` for the gallery, as the specimen does. A normal build answers 404.
 */
export function load(): void {
  if (!dev && import.meta.env['VITE_ANIMA_SPECIMEN'] !== '1') error(404, 'Not found');
}
