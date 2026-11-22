import type { CaretAt, Chip, ChipReader } from '../../draft';
import { outsideChips, soundChips, wholeChips } from '../chips';
import { applied, type Edit, type Mode, type Span } from '../edit';
import { NO_HISTORY, recorded, redone, undone, type ChangeKind, type History, type Moment } from '../history/history';
import type { Action } from '../keys/chords';
import { lines } from '../markdown/blocks';
import type { Desk } from '../typing';
import { caretRect, keepCaretInView, select, selectionIn } from './caret';
import { paint, type Drawn, type IconDrawer } from './draw';
import { offsetOf, read } from './points';
import { sketch } from './sketch';

/** What the sheet needs from the editor's component. Mode and chips are asked for at each drawing. */
export interface SheetPorts {
  readonly root: HTMLElement;
  readonly mode: () => Mode;
  readonly chips: () => ChipReader | undefined;
  readonly icon: IconDrawer;
  readonly now: () => number;
  /** The prompt changed in the editor. */
  readonly changed: (value: string) => void;
  /** The prompt or the selection changed. */
  readonly moved: (at: CaretAt) => void;
}

/**
 * The prompt on the page. The Markdown string is the one truth (03, The editor: What it is): every change is made
 * on the string, then the page is drawn from it and the caret is put back by its offset. The sheet keeps its own
 * undo, as a drawing breaks the browser's.
 */
export class Sheet {
  value = '';
  anchor = 0;
  focus = 0;
  #history: History = NO_HISTORY;
  #drawn: readonly Drawn[] = [];
  #chips: readonly Chip[] = [];
  readonly #ports: SheetPorts;

  constructor(ports: SheetPorts) {
    this.#ports = ports;
  }

  get sel(): Span {
    return { start: Math.min(this.anchor, this.focus), end: Math.max(this.anchor, this.focus) };
  }

  get desk(): Desk {
    return { value: this.value, sel: this.sel, mode: this.#ports.mode(), chips: this.#chips };
  }

  get focused(): boolean {
    return this.#ports.root.ownerDocument.activeElement === this.#ports.root;
  }

  /** The selection's Markdown, with whole chips: what copy and cut write (03, The editor: Markdown round trip). */
  get picked(): Span {
    return wholeChips(this.sel, this.#chips);
  }

  /** Takes a prompt from outside: another draft, or the prompt cleared after a send. Its history starts again. */
  load(value: string): void {
    if (value === this.value) return;
    this.value = value;
    this.anchor = value.length;
    this.focus = value.length;
    this.#history = NO_HISTORY;
    this.show();
  }

  apply(change: Edit, kind: ChangeKind): void {
    const next = applied(this.value, change);
    if (next !== this.value) {
      this.#history = recorded(this.#history, this.#moment(), kind, this.#ports.now());
      this.value = next;
      this.#ports.changed(next);
    }
    this.anchor = change.anchor;
    this.focus = change.focus;
    this.show();
  }

  act(action: Action): void {
    if (action === null) return;
    if (action !== 'undo' && action !== 'redo') {
      this.apply(action.edit, action.kind);
      return;
    }
    const step = action === 'undo' ? undone(this.#history, this.#moment()) : redone(this.#history, this.#moment());
    if (step === null) return;
    this.#history = step.history;
    ({ value: this.value, anchor: this.anchor, focus: this.focus } = step.moment);
    this.#ports.changed(this.value);
    this.show();
  }

  /** Draws the prompt and puts the caret where the sheet has it. */
  show(fresh = false): void {
    this.#draw(fresh);
    if (this.focused) {
      select(this.#ports.root, this);
      keepCaretInView(this.#ports.root);
    }
    this.#tell();
  }

  /** Takes the selection from the page, without drawing: before a key or an input is read. */
  heed(): boolean {
    const found = selectionIn(this.#ports.root);
    if (found === null) return false;
    const anchor = outsideChips(found.anchor, this.#chips);
    const focus = outsideChips(found.focus, this.#chips);
    const snapped = anchor !== found.anchor || focus !== found.focus;
    const moved = anchor !== this.anchor || focus !== this.focus;
    this.anchor = anchor;
    this.focus = focus;
    return snapped || moved;
  }

  /**
   * The selection moved in the page, or focus came or went: the markers the caret now touches show, the others
   * hide (03, The editor: How Markdown markers are shown and hidden). `always` draws even when nothing moved.
   */
  follow(always = false): void {
    if (!this.heed() && !always) return;
    // The browser's selection is written only after a drawing, so a caret that moves by lines keeps its column.
    if (this.#draw(false) && this.focused) select(this.#ports.root, this);
    this.#tell();
  }

  /** The browser changed the page itself, as an input method does: the prompt is read back from the page. */
  absorb(): void {
    const text = read(this.#ports.root);
    if (text === this.value) return;
    this.#history = recorded(this.#history, this.#moment(), 'insert', this.#ports.now());
    const found = selectionIn(this.#ports.root);
    this.value = text;
    this.anchor = Math.min(found?.anchor ?? text.length, text.length);
    this.focus = Math.min(found?.focus ?? text.length, text.length);
    this.#ports.changed(text);
    this.show(true);
  }

  /** The stretch of the prompt between two places of the page, or null when one is outside the editor. */
  span(start: { node: Node; offset: number }, end: { node: Node; offset: number }): Span | null {
    const from = offsetOf(this.#ports.root, start.node, start.offset);
    const to = offsetOf(this.#ports.root, end.node, end.offset);
    return from === null || to === null ? null : { start: Math.min(from, to), end: Math.max(from, to) };
  }

  /** The stretch a part of the page stands for: a chip, a line. */
  around(node: Node): Span | null {
    const parent = node.parentNode;
    if (parent === null) return null;
    const index = Array.prototype.indexOf.call(parent.childNodes, node);
    return this.span({ node: parent, offset: index }, { node: parent, offset: index + 1 });
  }

  /** Sets the selection, without drawing. */
  place(anchor: number, focus = anchor): void {
    const limit = this.value.length;
    this.anchor = Math.max(0, Math.min(anchor, limit));
    this.focus = Math.max(0, Math.min(focus, limit));
  }

  /** Frees what the lines hold, when the editor goes. */
  clear(): void {
    for (const line of this.#drawn) line.release();
    this.#drawn = [];
  }

  #moment(): Moment {
    return { value: this.value, anchor: this.anchor, focus: this.focus };
  }

  #draw(fresh: boolean): boolean {
    const { root } = this.#ports;
    const mode = this.#ports.mode();
    this.#chips = mode === 'literal' ? [] : soundChips(this.value, this.#ports.chips()?.(this.value) ?? []);
    const look = { mode, chips: this.#chips, sel: this.focused ? this.sel : null };
    if (fresh) {
      this.clear();
      root.replaceChildren();
    }
    const painted = paint(root, this.#drawn, lines(this.value, mode).map((line) => sketch(line, look)), this.#ports.icon);
    this.#drawn = painted.drawn;
    return painted.changed;
  }

  #tell(): void {
    const bare = this.focused && this.anchor === this.focus;
    this.#ports.moved({ prompt: this.value, caret: this.focus, rect: bare ? caretRect(this.#ports.root) : null });
  }
}
