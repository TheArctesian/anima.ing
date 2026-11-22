import type { CodeIconName } from '../../../icons/icons';
import type { Bit, Sketch } from './sketch';

/** Draws an icon into `target` and gives back how to take it away. The editor's component brings it, with `CodeIcon`. */
export type IconDrawer = (target: HTMLElement, name: CodeIconName) => () => void;

/** One line as it is in the page. */
export interface Drawn {
  readonly key: string;
  readonly element: HTMLElement;
  /** Takes away what the line holds that the page does not free by itself: its icons. */
  readonly release: () => void;
}

/** Bullets by depth, as T3's disc, circle and square (03, The editor: Node CSS). */
const BULLETS = ['•', '◦', '▪'];
/** The words a code block with no language shows (03, The editor: Language picker, its first row). */
export const PLAIN_CODE = 'Plain text';
/** The name of a task's checkbox for assistive technology. */
export const TASK_LABEL = 'Done';

function element<Tag extends keyof HTMLElementTagNameMap>(tag: Tag, classes: readonly string[]): HTMLElementTagNameMap[Tag] {
  const made = document.createElement(tag);
  made.className = classes.join(' ');
  return made;
}

/** A chip: one unit that is not edited, with its source kept on it so the prompt can be read back from the page. */
function chipNode(bit: Bit, icon: IconDrawer, releases: (() => void)[]): HTMLElement {
  const chip = element('span', ['ce-chip', ...(bit.picked === null ? [] : [`ce-${bit.picked}`])]);
  chip.setAttribute('contenteditable', 'false');
  chip.dir = 'ltr';
  chip.dataset.source = bit.text;
  if (bit.chip === null) return chip;
  chip.title = bit.chip.detail;
  chip.setAttribute('role', 'img');
  chip.setAttribute('aria-label', bit.chip.detail);
  releases.push(icon(chip.appendChild(element('span', ['ce-chip-icon'])), bit.chip.icon));
  chip.appendChild(element('span', ['ce-chip-label'])).textContent = bit.chip.label;
  return chip;
}

function bitNode(bit: Bit, icon: IconDrawer, releases: (() => void)[]): Node {
  if (bit.role === 'chip') return chipNode(bit, icon, releases);
  if (bit.role === 'marker') {
    const marker = element('span', bit.hidden ? ['ce-mark', 'ce-off'] : ['ce-mark']);
    marker.textContent = bit.text;
    return marker;
  }
  if (bit.styles.length === 0) return document.createTextNode(bit.text);
  // Inline code is always left to right, and does not count for the direction of its line (03, The editor: Text direction).
  const styled = element(bit.styles.includes('code') ? 'code' : 'span', bit.styles.map((style) => `ce-${style}`));
  if (bit.styles.includes('code')) styled.dir = 'ltr';
  styled.textContent = bit.text;
  return styled;
}

function checkbox(checked: boolean): HTMLInputElement {
  const box = element('input', ['ce-check']);
  box.type = 'checkbox';
  box.checked = checked;
  box.tabIndex = -1;
  box.setAttribute('contenteditable', 'false');
  box.setAttribute('aria-label', TASK_LABEL);
  return box;
}

function classes(sketch: Sketch): string[] {
  const { line } = sketch;
  const edges = [...(line.first ? ['ce-first'] : []), ...(line.last ? ['ce-last'] : [])];
  return ['ce-line', `ce-${line.kind}`, ...(line.level > 0 ? [`ce-h${String(line.level)}`] : []), ...edges, ...(sketch.source ? ['ce-source'] : [])];
}

function dress(made: HTMLElement, sketch: Sketch): void {
  const { line } = sketch;
  const coded = line.block !== -1;
  // Each line finds its own direction from its text; code is always left to right (03, The editor: Text direction).
  made.dir = coded ? 'ltr' : 'auto';
  if (line.kind === 'fence' && line.prefix === line.text.length) made.dataset.plain = PLAIN_CODE;
  if (line.kind !== 'bullet' && line.kind !== 'ordered' && line.kind !== 'task') return;
  made.style.setProperty('--code-composer-editor-depth', String(line.depth));
  if (line.kind === 'task') return;
  made.dataset.marker = line.kind === 'ordered' ? line.marker : (BULLETS[Math.min(line.depth, BULLETS.length - 1)] ?? '');
  made.style.setProperty('--code-composer-editor-marker', String(line.gutter));
}

/** The element of one line: every character of the line is in it, shown or hidden, in order. */
export function drawLine(sketch: Sketch, icon: IconDrawer): Drawn {
  const { line, bits } = sketch;
  const releases: (() => void)[] = [];
  const made = element('div', classes(sketch));
  dress(made, sketch);
  for (const bit of bits) {
    made.append(bitNode(bit, icon, releases));
    if (line.kind === 'task' && bit.role === 'marker' && bit.start === 0) made.append(checkbox(line.checked));
  }
  // A line with nothing to show still needs a place for the caret; the hidden closing fence has none.
  const shown = bits.some((bit) => !bit.hidden);
  if (!shown && (line.kind !== 'close' || sketch.source)) made.append(document.createElement('br'));
  return { key: sketch.key, element: made, release: () => { for (const release of releases) release(); } };
}

/**
 * Makes the page show `sketches`. Lines at the top and the bottom that did not change keep their elements, so typing
 * redraws one line. Says whether anything was drawn.
 */
export function paint(root: HTMLElement, before: readonly Drawn[], sketches: readonly Sketch[], icon: IconDrawer): { drawn: Drawn[]; changed: boolean } {
  let head = 0;
  while (head < before.length && head < sketches.length && before[head]?.key === sketches[head]?.key) head += 1;
  let tail = 0;
  const most = Math.min(before.length, sketches.length) - head;
  while (tail < most && before[before.length - 1 - tail]?.key === sketches[sketches.length - 1 - tail]?.key) tail += 1;
  const stale = before.slice(head, before.length - tail);
  const fresh = sketches.slice(head, sketches.length - tail).map((sketch) => drawLine(sketch, icon));
  for (const old of stale) {
    old.release();
    old.element.remove();
  }
  const anchor = before[before.length - tail]?.element ?? null;
  for (const made of fresh) root.insertBefore(made.element, tail === 0 ? null : anchor);
  return { drawn: [...before.slice(0, head), ...fresh, ...before.slice(before.length - tail)], changed: stale.length + fresh.length > 0 };
}
