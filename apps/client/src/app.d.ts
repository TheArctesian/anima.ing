// See https://svelte.dev/docs/kit/types#app.d.ts
declare global {
  namespace App {
    /** State a page passes to the next one with `goto(url, { state })`; it is not in the URL, so a reload does not replay it. */
    interface PageState {
      /** The hub's message when a new device was created but its credential was not stored (WP-13 H-1). Never the credential. */
      credentialError?: string;
    }
  }
}

declare module '$app/paths' {
  /**
   * A path of the app with the base before it. TypeScript checks a union of paths against the
   * signature of SvelteKit only up to 25 members, and the app has more; this one takes each path.
   */
  export function resolve(path: import('$app/types').Pathname): string;
}

export {};
