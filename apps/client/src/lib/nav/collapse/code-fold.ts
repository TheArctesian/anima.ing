/**
 * While the page Code is open, Anima's bar folds to its icons (OD-THR60), so that the inbox and the chat have the width
 * of the window. The fold is for the visit: the choice the person stored stays. Opening the bar on Code keeps it open
 * until the person leaves Code. Pure.
 */

/** Whether the bar folds on the page at `pathname`. The specimen `/code-kit` is not the page. */
export function foldsBar(pathname: string): boolean {
  return pathname === '/code' || pathname.startsWith('/code/');
}

/** What the bar shows: the stored choice, or folded on Code unless the person opened it there. */
export interface BarFold {
  /** The choice the person stored on this device. */
  readonly stored: boolean;
  /** The person opened the bar on this visit of Code. */
  readonly openedHere: boolean;
}

/** Whether the bar shows folded on `pathname`. */
export function shownFolded(fold: BarFold, pathname: string): boolean {
  return fold.stored || (foldsBar(pathname) && !fold.openedHere);
}

/**
 * The fold after the person folds (`next` true) or opens the bar on `pathname`. Opening is stored, as anywhere; folding
 * on Code only ends the visit's opening, since Code folds the bar anyway.
 */
export function afterToggle(fold: BarFold, next: boolean, pathname: string): { readonly fold: BarFold; readonly store: boolean } {
  if (!foldsBar(pathname)) return { fold: { stored: next, openedHere: false }, store: true };
  if (!next) return { fold: { stored: false, openedHere: true }, store: fold.stored };
  return { fold: { stored: fold.stored, openedHere: false }, store: false };
}

/** The fold after leaving Code: the visit's opening ends. */
export function afterLeaving(fold: BarFold, pathname: string): BarFold {
  return foldsBar(pathname) || !fold.openedHere ? fold : { stored: fold.stored, openedHere: false };
}
