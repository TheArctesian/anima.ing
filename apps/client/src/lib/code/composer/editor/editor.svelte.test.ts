// @vitest-environment jsdom
import { flushSync, mount, unmount, type ComponentProps } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { CaretAt, Chip } from '../draft';
import Editor from './Editor.svelte';
import { LONG_PASTE_BYTES } from './typing';
import { pointAt } from './view/points';

type Props = ComponentProps<typeof Editor>;

let app: ReturnType<typeof mount> | undefined;

/** The editor in a page, with its prompt bound to `props.value`, as the composer holds it. */
function editor(start: Partial<Props> = {}) {
  const props: Props = $state({ value: '', placeholder: 'Ask anything', label: 'Message', ...start });
  const made = mount(Editor, { target: document.body, props });
  app = made;
  flushSync();
  const box = document.querySelector<HTMLElement>('[role="textbox"]');
  if (box === null) throw new Error('no text box');
  return { props, box, app: made };
}

function select(box: HTMLElement, anchor: number, focus = anchor): void {
  const from = pointAt(box, anchor);
  const to = pointAt(box, focus);
  document.getSelection()?.setBaseAndExtent(from.node, from.offset, to.node, to.offset);
  document.dispatchEvent(new Event('selectionchange'));
  flushSync();
}

function input(box: HTMLElement, inputType: string, data: string | null = null, isComposing = false): InputEvent {
  const event = new InputEvent('beforeinput', { inputType, data, isComposing, bubbles: true, cancelable: true });
  box.dispatchEvent(event);
  flushSync();
  return event;
}

function type(box: HTMLElement, text: string): void {
  for (const char of text) input(box, 'insertText', char);
}

function press(box: HTMLElement, key: string, more: KeyboardEventInit = {}): KeyboardEvent {
  const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...more });
  box.dispatchEvent(event);
  flushSync();
  return event;
}

function clipboard(box: HTMLElement, type: 'paste' | 'copy' | 'cut', held: { text?: string; files?: File[] } = {}) {
  const written = new Map<string, string>();
  const event = new Event(type, { bubbles: true, cancelable: true });
  const clipboardData = { files: held.files ?? [], getData: () => held.text ?? '', setData: (kind: string, text: string) => written.set(kind, text) };
  Object.defineProperty(event, 'clipboardData', { value: clipboardData });
  box.dispatchEvent(event);
  flushSync();
  return { event, written };
}

/** What a reader sees: the text of the page without the markers that are not drawn. */
function seen(box: HTMLElement): string {
  const copy = box.cloneNode(true) as HTMLElement;
  for (const hidden of copy.querySelectorAll('.ce-off')) hidden.remove();
  return Array.from(copy.children, (line) => line.textContent).join('\n');
}

afterEach(async () => {
  if (app !== undefined) await unmount(app);
  app = undefined;
  document.body.replaceChildren();
  document.getSelection()?.removeAllRanges();
});

describe('the text box', () => {
  it('is a named multi-line text box with its placeholder over it while it is empty', () => {
    const { box, props } = editor();
    expect(box.getAttribute('aria-label')).toBe('Message');
    expect(box.getAttribute('aria-multiline')).toBe('true');
    expect(box.getAttribute('aria-placeholder')).toBe('Ask anything');
    expect(box.getAttribute('contenteditable')).toBe('true');
    expect(document.querySelector('.code-editor-placeholder')?.textContent).toBe('Ask anything');
    props.value = 'hello';
    flushSync();
    expect(document.querySelector('.code-editor-placeholder')).toBeNull();
    expect(seen(box)).toBe('hello');
  });

  it('can be read and not changed while it is disabled', () => {
    const { box, props } = editor({ value: 'fixed', disabled: true });
    expect(box.getAttribute('aria-readonly')).toBe('true');
    expect(box.getAttribute('contenteditable')).toBe('false');
    box.focus();
    select(box, 5);
    expect(input(box, 'insertText', 'x').defaultPrevented).toBe(true);
    expect(clipboard(box, 'paste', { text: 'more' }).event.defaultPrevented).toBe(true);
    expect(props.value).toBe('fixed');
  });

  it('draws Markdown, and shows the markers only of the styled text the caret touches', () => {
    const { box } = editor({ value: '# Title\na **bold** word' });
    expect(seen(box)).toBe('Title\na bold word');
    expect(box.querySelector('.ce-h1')?.textContent).toBe('# Title');
    box.focus();
    select(box, 12);
    expect(seen(box)).toBe('Title\na **bold** word');
    select(box, 0);
    expect(seen(box)).toBe('Title\na bold word');
  });

  it('draws only text in literal mode', () => {
    const { box } = editor({ value: '# a **b**', mode: 'literal' });
    expect(seen(box)).toBe('# a **b**');
    expect(box.querySelector('.ce-heading')).toBeNull();
  });
});

describe('typing', () => {
  it('changes the bound prompt, refuses the browser its own edit, and tells where the caret is', () => {
    const onCaret = vi.fn<(at: CaretAt) => void>();
    const { box, props } = editor({ onCaret });
    box.focus();
    expect(input(box, 'insertText', 'h').defaultPrevented).toBe(true);
    type(box, 'i @');
    expect(props.value).toBe('hi @');
    expect(onCaret).toHaveBeenLastCalledWith(expect.objectContaining({ prompt: 'hi @', caret: 4 }));
    expect(onCaret.mock.lastCall?.[0].rect).not.toBeNull();
    select(box, 0, 2);
    expect(onCaret).toHaveBeenLastCalledWith({ prompt: 'hi @', caret: 2, rect: null });
  });

  it('types at the caret the reader placed, and deletes there', () => {
    const { box, props } = editor({ value: 'ac' });
    box.focus();
    select(box, 1);
    type(box, 'b');
    expect(props.value).toBe('abc');
    input(box, 'deleteContentBackward');
    input(box, 'deleteContentForward');
    expect(props.value).toBe('a');
  });

  it('turns typed Markdown into blocks as it is typed', () => {
    const { box, props } = editor();
    box.focus();
    type(box, '- [ ] buy');
    expect(props.value).toBe('- [ ] buy');
    expect(box.querySelector('.ce-task input[type="checkbox"]')).not.toBeNull();
    expect(seen(box)).toBe('buy');
  });

  it('undoes and redoes its own steps', () => {
    const { box, props } = editor();
    box.focus();
    type(box, 'one');
    clipboard(box, 'paste', { text: ' two' });
    expect(props.value).toBe('one two');
    press(box, 'z', { ctrlKey: true });
    expect(props.value).toBe('one');
    press(box, 'z', { ctrlKey: true });
    expect(props.value).toBe('');
    press(box, 'z', { ctrlKey: true, shiftKey: true });
    input(box, 'historyRedo');
    expect(props.value).toBe('one two');
  });
});

describe('keys', () => {
  it('offers every key to the composer first, and stops a key it took', () => {
    const onKey = vi.fn((event: KeyboardEvent) => event.key === 'Enter' && !event.shiftKey);
    const { box, props } = editor({ value: '- a', onKey });
    box.focus();
    select(box, 3);
    expect(press(box, 'Enter').defaultPrevented).toBe(true);
    expect(props.value).toBe('- a');
    expect(press(box, 'Enter', { shiftKey: true }).defaultPrevented).toBe(true);
    expect(props.value).toBe('- a\n- ');
    expect(onKey).toHaveBeenCalledTimes(2);
  });

  it('leaves Tab to move focus where it has no rule', () => {
    const { box } = editor({ value: 'text' });
    box.focus();
    select(box, 4);
    expect(press(box, 'Tab').defaultPrevented).toBe(false);
  });

  it('styles the selection with the Mod keys', () => {
    const { box, props } = editor({ value: 'make bold' });
    box.focus();
    select(box, 5, 9);
    press(box, 'b', { ctrlKey: true });
    expect(props.value).toBe('make **bold**');
  });

  it('does nothing while an input method composes, then reads its text from the page', () => {
    const onKey = vi.fn(() => false);
    const { box, props } = editor({ value: '- ', onKey });
    box.focus();
    select(box, 2);
    box.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
    expect(press(box, 'Enter', { isComposing: true }).defaultPrevented).toBe(false);
    expect(press(box, 'Process', { keyCode: 229 }).defaultPrevented).toBe(false);
    expect(input(box, 'insertCompositionText', 'か', true).defaultPrevented).toBe(false);
    expect(onKey).not.toHaveBeenCalled();
    const caret = document.getSelection();
    caret?.anchorNode?.insertBefore(document.createTextNode('漢字'), caret.anchorNode.childNodes[caret.anchorOffset] ?? null);
    box.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true }));
    flushSync();
    expect(props.value).toBe('- 漢字');
    expect(seen(box)).toBe('漢字');
    press(box, 'z', { ctrlKey: true });
    expect(props.value).toBe('- ');
  });
});

describe('the clipboard', () => {
  it('hands pasted files up and puts nothing in when they are taken', () => {
    const onFiles = vi.fn(() => true);
    const { box, props } = editor({ onFiles });
    box.focus();
    const file = new File(['x'], 'shot.png');
    expect(clipboard(box, 'paste', { files: [file], text: 'shot.png' }).event.defaultPrevented).toBe(true);
    expect(onFiles).toHaveBeenCalledWith([file], 'paste');
    expect(props.value).toBe('');
  });

  it('puts the pasted text in when the files are not taken', () => {
    const { box, props } = editor({ onFiles: () => false });
    box.focus();
    clipboard(box, 'paste', { files: [new File(['x'], 'cells.png')], text: 'a\tb' });
    expect(props.value).toBe('a\tb');
  });

  it('puts pasted text in as Markdown, with one kind of line break', () => {
    const { box, props } = editor({ value: 'a' });
    box.focus();
    select(box, 1);
    clipboard(box, 'paste', { text: ' **b**\r\n- c' });
    expect(props.value).toBe('a **b**\n- c');
  });

  it('hands a long paste up in place of putting it in', () => {
    const onLongPaste = vi.fn();
    const { box, props } = editor({ onLongPaste });
    box.focus();
    const long = 'x'.repeat(LONG_PASTE_BYTES);
    clipboard(box, 'paste', { text: long });
    expect(onLongPaste).toHaveBeenCalledWith(long);
    expect(props.value).toBe('');
  });

  it('copies and cuts the Markdown of the selection', () => {
    const { box, props } = editor({ value: 'a **bold** word' });
    box.focus();
    select(box, 2, 10);
    expect(clipboard(box, 'copy').written.get('text/plain')).toBe('**bold**');
    expect(clipboard(box, 'cut').written.get('text/plain')).toBe('**bold**');
    expect(props.value).toBe('a  word');
  });
});

describe('what the composer calls', () => {
  it('replaces a stretch, as for a mention, and leaves the caret after it', () => {
    const onCaret = vi.fn<(at: CaretAt) => void>();
    const { app: made, props } = editor({ value: 'see @fi now', onCaret });
    made.replace(4, 7, '[file.ts](src/file.ts) ');
    flushSync();
    expect(props.value).toBe('see [file.ts](src/file.ts)  now');
    expect(onCaret).toHaveBeenLastCalledWith(expect.objectContaining({ caret: 27 }));
    made.focus();
    made.insert('!');
    flushSync();
    expect(props.value).toBe('see [file.ts](src/file.ts) ! now');
    expect(document.activeElement?.getAttribute('role')).toBe('textbox');
  });

  it('takes a new prompt from outside and starts its history again', () => {
    const { box, props } = editor();
    box.focus();
    type(box, 'sent');
    props.value = '';
    flushSync();
    press(box, 'z', { ctrlKey: true });
    expect(props.value).toBe('');
    type(box, 'next');
    expect(props.value).toBe('next');
  });
});

describe('chips and tasks', () => {
  const SOURCE = '[a.ts](src/a.ts)';
  const chips = (prompt: string): Chip[] => {
    const at = prompt.indexOf(SOURCE);
    return at === -1 ? [] : [{ start: at, end: at + SOURCE.length, label: 'a.ts', icon: 'file', detail: 'src/a.ts' }];
  };

  it('draws a chip with its icon and name, and removes it whole', () => {
    const { box, props } = editor({ value: `see ${SOURCE} now`, chips });
    const chip = box.querySelector<HTMLElement>('.ce-chip');
    expect(chip?.textContent).toBe('a.ts');
    expect(chip?.title).toBe('src/a.ts');
    expect(chip?.querySelector('svg')).not.toBeNull();
    box.focus();
    select(box, 4 + SOURCE.length);
    input(box, 'deleteContentBackward');
    expect(props.value).toBe('see  now');
    expect(box.querySelector('.ce-chip')).toBeNull();
  });

  it('selects a chip as one unit when it is pressed', () => {
    const onCaret = vi.fn<(at: CaretAt) => void>();
    const { box } = editor({ value: `see ${SOURCE} now`, chips, onCaret });
    box.querySelector('.ce-chip-label')?.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, cancelable: true }));
    flushSync();
    expect(box.querySelector('.ce-chip')?.classList.contains('ce-alone')).toBe(true);
    expect(clipboard(box, 'copy').written.get('text/plain')).toBe(SOURCE);
  });

  it('ticks a task in the Markdown when its box is clicked', () => {
    const { box, props } = editor({ value: 'list\n- [ ] one\n- [x] two' });
    const boxes = box.querySelectorAll<HTMLInputElement>('input.ce-check');
    boxes[0]?.click();
    flushSync();
    expect(props.value).toBe('list\n- [x] one\n- [x] two');
    box.querySelectorAll<HTMLInputElement>('input.ce-check')[1]?.click();
    flushSync();
    expect(props.value).toBe('list\n- [x] one\n- [ ] two');
    expect(box.querySelectorAll<HTMLInputElement>('input.ce-check')[1]?.checked).toBe(false);
  });
});
