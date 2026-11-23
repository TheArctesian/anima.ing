/** On Code the bar folds to its icons for the visit; the stored choice stays (OD-THR60). */

/** The specimen `/code-kit` is not the page Code. */
export function foldsBar(pathname: string): boolean {
  return pathname === '/code' || pathname.startsWith('/code/');
}

export interface BarFold {
  readonly stored: boolean;
  readonly openedHere: boolean;
}

export function shownFolded(fold: BarFold, pathname: string): boolean {
  return fold.stored || (foldsBar(pathname) && !fold.openedHere);
}

/** Folding on Code only ends the visit's opening, since Code folds the bar anyway. */
export function afterToggle(fold: BarFold, next: boolean, pathname: string): { readonly fold: BarFold; readonly store: boolean } {
  if (!foldsBar(pathname)) return { fold: { stored: next, openedHere: false }, store: true };
  if (!next) return { fold: { stored: false, openedHere: true }, store: fold.stored };
  return { fold: { stored: fold.stored, openedHere: false }, store: false };
}

export function afterLeaving(fold: BarFold, pathname: string): BarFold {
  return foldsBar(pathname) || !fold.openedHere ? fold : { stored: fold.stored, openedHere: false };
}
