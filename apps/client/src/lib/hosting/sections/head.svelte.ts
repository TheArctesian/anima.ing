import { getContext, setContext, type Snippet } from 'svelte';

/** The actions of the Hosting page header, which the open section puts there (Add a route for Routes). */
export class HostingHead {
  actions = $state<Snippet | undefined>();
}

const KEY = Symbol('hosting head');

/** Gives the sections of Hosting the header of the page. */
export function provideHead(): HostingHead {
  return setContext(KEY, new HostingHead());
}

/** The header of the Hosting page, for a section to put its actions in. */
export function useHead(): HostingHead {
  return getContext<HostingHead>(KEY);
}
