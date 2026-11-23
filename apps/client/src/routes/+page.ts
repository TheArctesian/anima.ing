import { redirect } from '@sveltejs/kit';

/* The static build has no server; a `load` redirect runs before render, where a `goto` in `onMount` would flash a blank page. */
export function load(): never {
  redirect(307, '/devices');
}
