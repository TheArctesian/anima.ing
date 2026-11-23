import { FOCUSABLE } from './focus';

export function clickedControl(target: EventTarget | null): HTMLElement | null {
  return target instanceof Element ? target.closest<HTMLElement>(FOCUSABLE) : null;
}

/** WebKit does not focus a button on click, so `active` may be a container; a clicked control inside it wins. */
export function openerOf(active: Element | null, clicked: HTMLElement | null): HTMLElement | null {
  if (clicked?.isConnected === true && (active === null || active.contains(clicked))) return clicked;
  return active instanceof HTMLElement ? active : null;
}

/** A key press forgets the click, so a dialog opened from the keyboard never takes an old click. */
let clicked: HTMLElement | null = null;
let watchers = 0;

const record = (event: MouseEvent) => {
  clicked = clickedControl(event.target);
};
const forget = () => {
  clicked = null;
};

/** It runs while the dialog is closed, because the click that opens it comes first. */
export function watchClicks(): () => void {
  if (watchers === 0) {
    document.addEventListener('click', record, { capture: true });
    document.addEventListener('keydown', forget, { capture: true });
  }
  watchers += 1;
  return () => {
    watchers -= 1;
    if (watchers > 0) return;
    document.removeEventListener('click', record, { capture: true });
    document.removeEventListener('keydown', forget, { capture: true });
    clicked = null;
  };
}

export function takeOpener(active: Element | null): HTMLElement | null {
  const opener = openerOf(active, clicked);
  clicked = null;
  return opener;
}
