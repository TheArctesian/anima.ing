import { offsetOf, pointAt } from './points';

/** The selection as offsets in the prompt: where it was started and where it is moved from. */
export interface Selected {
  readonly anchor: number;
  readonly focus: number;
}

/** The browser's selection as offsets in the prompt, or null when it is not all inside `root`. */
export function selectionIn(root: HTMLElement): Selected | null {
  const selection = root.ownerDocument.getSelection();
  if (selection === null || selection.anchorNode === null || selection.focusNode === null) return null;
  const anchor = offsetOf(root, selection.anchorNode, selection.anchorOffset);
  const focus = offsetOf(root, selection.focusNode, selection.focusOffset);
  return anchor === null || focus === null ? null : { anchor, focus };
}

/** Puts the browser's selection at `selected`. It is left alone when it is there already, so the caret keeps its column. */
export function select(root: HTMLElement, selected: Selected): void {
  const selection = root.ownerDocument.getSelection();
  if (selection === null) return;
  const anchor = pointAt(root, selected.anchor);
  const focus = selected.focus === selected.anchor ? anchor : pointAt(root, selected.focus);
  const same =
    selection.anchorNode === anchor.node &&
    selection.anchorOffset === anchor.offset &&
    selection.focusNode === focus.node &&
    selection.focusOffset === focus.offset;
  if (!same) selection.setBaseAndExtent(anchor.node, anchor.offset, focus.node, focus.offset);
}

/** Where the caret is on screen, for a popup to anchor on; null when there is no bare caret in `root`. */
export function caretRect(root: HTMLElement): DOMRect | null {
  const selection = root.ownerDocument.getSelection();
  if (selection === null || selection.rangeCount === 0 || !selection.isCollapsed) return null;
  const range = selection.getRangeAt(0);
  if (!root.contains(range.startContainer)) return null;
  // A test page has no layout, and so no rectangles of a range.
  const first = typeof range.getClientRects === 'function' ? range.getClientRects()[0] : undefined;
  if (first !== undefined) return first;
  // On an empty line a caret has no rectangle of its own: the line's edge stands for it.
  const holder = range.startContainer instanceof Element ? range.startContainer : range.startContainer.parentElement;
  const box = (holder ?? root).getBoundingClientRect();
  return new DOMRect(getComputedStyle(holder ?? root).direction === 'rtl' ? box.right : box.left, box.top, 0, box.height);
}

/** Scrolls the editor, and never the page, until the caret is inside it (03, The editor: Height, growth, scrolling). */
export function keepCaretInView(root: HTMLElement): void {
  const rect = caretRect(root);
  if (rect === null) return;
  const box = root.getBoundingClientRect();
  if (rect.bottom > box.bottom) root.scrollTop += rect.bottom - box.bottom;
  else if (rect.top < box.top) root.scrollTop -= box.top - rect.top;
}

/** The offset in the prompt under a point of the screen, for a drop; null when the browser cannot say. */
export function offsetUnder(root: HTMLElement, x: number, y: number): number | null {
  const page = root.ownerDocument;
  if (typeof page.caretPositionFromPoint === 'function') {
    const place = page.caretPositionFromPoint(x, y);
    return place === null ? null : offsetOf(root, place.offsetNode, place.offset);
  }
  // WebKit has only the older name.
  const range = typeof page.caretRangeFromPoint === 'function' ? page.caretRangeFromPoint(x, y) : null;
  return range === null ? null : offsetOf(root, range.startContainer, range.startOffset);
}
