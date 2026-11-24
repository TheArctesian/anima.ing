declare global {
  namespace App {
    /** Not in the URL, so a reload does not replay it. */
    interface PageState {
      /** The hub's message when a new device's credential was not stored; never the credential (WP-13 H-1). */
      credentialError?: string;
    }
  }
}

declare module '$app/paths' {
  /** SvelteKit's signature checks a union of paths only up to 25 members, and the app has more. */
  export function resolve(path: import('$app/types').Pathname): string;
}

export {};
