import { getContext, setContext, type Snippet } from 'svelte';

/** Keeps a form's actions in view below the scrolling body; it shows the actions of the form that showed them last. */
interface ModalFooter {
  /** Shows `actions` in the footer; the returned function takes them out again. */
  readonly show: (actions: Snippet) => () => void;
}

const key = Symbol('modal footer');

export function offerFooter(footer: ModalFooter): void {
  setContext(key, footer);
}

/** `undefined` outside a modal, where actions stay in place. */
export function modalFooter(): ModalFooter | undefined {
  return getContext<ModalFooter | undefined>(key);
}
