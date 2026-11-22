import { error } from '@sveltejs/kit';
import { dev } from '$app/environment';

/**
 * The Git page of the kit of the Code section (stage 7 of the T3 Code copy) exists in development, and in a build made
 * with `VITE_ANIMA_SPECIMEN=1` for the gallery, as the specimen does. A normal build answers 404.
 */
export function load({ url }: { url: URL }): { dialog: string | null } {
  if (!dev && import.meta.env['VITE_ANIMA_SPECIMEN'] !== '1') error(404, 'Not found');
  return { dialog: url.searchParams.get('dialog') };
}
