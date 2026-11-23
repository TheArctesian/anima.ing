import { redirect } from '@sveltejs/kit';

/* A link to Threads goes on to Code (operator ruling, 2026-11-21). */
export function load(): never {
  redirect(308, '/code');
}
