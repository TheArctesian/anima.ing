/** A place in the page, as the browser's selection names it. */
export interface Point {
  readonly node: Node;
  readonly offset: number;
}

function sourceOf(node: Node): string | undefined {
  return node instanceof HTMLElement ? node.dataset.source : undefined;
}

/** The Markdown a part of the page stands for: its text, and for a chip its source. */
export function textOf(node: Node): string {
  if (node.nodeType === Node.TEXT_NODE) return node.nodeValue ?? '';
  const source = sourceOf(node);
  if (source !== undefined) return source;
  let text = '';
  for (const child of node.childNodes) text += textOf(child);
  return text;
}

function sizeOf(node: Node): number {
  if (node.nodeType === Node.TEXT_NODE) return node.nodeValue?.length ?? 0;
  const source = sourceOf(node);
  if (source !== undefined) return source.length;
  let size = 0;
  for (const child of node.childNodes) size += sizeOf(child);
  return size;
}

/** The prompt as the page has it: each child of `root` is one line. Read after the browser changed the page itself. */
export function read(root: HTMLElement): string {
  return Array.from(root.childNodes, textOf).join('\n');
}

function before(parent: Node, count: number): number {
  let size = 0;
  for (let index = 0; index < count && index < parent.childNodes.length; index += 1) size += sizeOf(parent.childNodes[index] as Node);
  return size;
}

/**
 * The offset in the prompt of a place in the page, or null when the place is outside `root`. A place inside a
 * chip counts as the chip's nearer end: the caret takes a chip as one position.
 */
export function offsetOf(root: HTMLElement, node: Node, offset: number): number | null {
  if (!root.contains(node)) return null;
  // Between the lines of the page, each line before the place adds its text and its line break.
  if (node === root) return before(root, offset) + Math.min(offset, Math.max(0, root.childNodes.length - 1));
  let at = node.nodeType === Node.TEXT_NODE ? offset : before(node, offset);
  let current = node;
  for (let parent = current.parentNode; parent !== null; parent = current.parentNode) {
    if (sourceOf(current) !== undefined) at = at === 0 ? 0 : sizeOf(current);
    const index = Array.prototype.indexOf.call(parent.childNodes, current);
    at += before(parent, index) + (parent === root ? index : 0);
    if (parent === root) return at;
    current = parent;
  }
  return null;
}

interface Leaf {
  readonly node: Node;
  readonly start: number;
  readonly size: number;
  /** Shown text takes the caret; a chip and a hidden marker are passed whole. */
  readonly text: boolean;
}

function leaves(node: Node, start: number, out: Leaf[]): number {
  let at = start;
  for (const child of node.childNodes) {
    const solid = child instanceof HTMLElement && (child.dataset.source !== undefined || child.classList.contains('ce-off'));
    const size = sizeOf(child);
    if (solid || child.nodeType === Node.TEXT_NODE) out.push({ node: child, start: at, size, text: !solid });
    else leaves(child, at, out);
    at += size;
  }
  return at;
}

function beside(node: Node, after: boolean): Point {
  const parent = node.parentNode as Node;
  return { node: parent, offset: Array.prototype.indexOf.call(parent.childNodes, node) + (after ? 1 : 0) };
}

function pointInLine(line: Node, offset: number): Point {
  if (line.nodeType === Node.TEXT_NODE) return { node: line, offset };
  const found: Leaf[] = [];
  leaves(line, 0, found);
  const text = found.find((leaf) => leaf.text && offset >= leaf.start && offset <= leaf.start + leaf.size);
  if (text !== undefined) return { node: text.node, offset: offset - text.start };
  // No shown text there: the place is beside a chip or a hidden marker, after it unless the offset is at its start.
  const solid = found.find((leaf) => leaf.start + leaf.size >= offset) ?? found.at(-1);
  return solid === undefined ? { node: line, offset: 0 } : beside(solid.node, offset > solid.start);
}

/** The place in the page of an offset in the prompt: in shown text where there is some, never inside a chip. */
export function pointAt(root: HTMLElement, offset: number): Point {
  let left = offset;
  for (const line of root.childNodes) {
    const size = sizeOf(line);
    if (left <= size) return pointInLine(line, left);
    left -= size + 1;
  }
  const last = root.lastChild;
  return last === null ? { node: root, offset: 0 } : pointInLine(last, sizeOf(last));
}
