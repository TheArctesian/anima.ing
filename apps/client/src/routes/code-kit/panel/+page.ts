import { error } from '@sveltejs/kit';
import { dev } from '$app/environment';

export function load(): void {
  if (!dev && import.meta.env['VITE_ANIMA_SPECIMEN'] !== '1') error(404, 'Not found');
}
