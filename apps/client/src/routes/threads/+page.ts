import { redirect } from '@sveltejs/kit';

/*
 * The page Threads is now the page Code (the ruling of the operator, 2026-11-21: "anima code"); an old link or
 * bookmark goes on to it. A `load` redirect, as the root page has: the client router follows it before it renders.
 */
export function load(): never {
  redirect(308, '/code');
}
