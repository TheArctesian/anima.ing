import { getContext, setContext, type Snippet } from 'svelte';

export class HostingHead {
  actions = $state<Snippet | undefined>();
}

const KEY = Symbol('hosting head');

export function provideHead(): HostingHead {
  return setContext(KEY, new HostingHead());
}

export function useHead(): HostingHead {
  return getContext<HostingHead>(KEY);
}
