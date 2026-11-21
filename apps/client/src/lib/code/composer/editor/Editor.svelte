<script lang="ts">
  import { mount, onMount, unmount, untrack } from 'svelte';
  import { isMac } from '$lib/platform/mac';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import type { CodeIconName } from '../../icons/icons';
  import '../../kit/code.css';
  import '../composer.css';
  import type { CaretAt, ChipReader } from '../draft';
  import { edit, type Mode } from './edit';
  import './editor.css';
  import { chordAction } from './keys/chords';
  import { ticked } from './lists/tasks';
  import { pasteIsLong, typedAction } from './typing';
  import { offsetUnder } from './view/caret';
  import { Sheet } from './view/sheet';

  interface Props {
    /** The prompt, as Markdown: the one truth of the editor (03, The editor: What it is). */
    value: string;
    placeholder: string;
    /** The accessible name of the text box: "Message". */
    label: string;
    /** It can be read and copied, not changed. */
    disabled?: boolean;
    /** `rich` draws Markdown; `plain` only paragraphs and chips; `literal` the bare text, for an answer to a question. */
    mode?: Mode;
    /** Finds the chips of a prompt: each is drawn as one unit. */
    chips?: ChipReader;
    /** Told at every change of the prompt or the selection. */
    onCaret?: (at: CaretAt) => void;
    /** Gets every key first; true means the composer took it, and the editor stops the key there. */
    onKey?: (event: KeyboardEvent) => boolean;
    /** Files pasted or dropped: the editor only hands them up. False: they were not taken, and pasted text goes in. */
    onFiles?: (files: readonly File[], how: 'paste' | 'drop') => boolean;
    /** The id of the list a trigger opened, and of its lit row, for assistive technology (03, The editor: Element tree). */
    controls?: string | undefined;
    activeDescendant?: string | undefined;
    /** Pasted text too long for the prompt, handed up in place of being put in. */
    onLongPaste?: (text: string) => void;
  }

  let { value = $bindable(), placeholder, label, disabled = false, mode = 'rich', chips, onCaret, onKey, onFiles, onLongPaste, controls, activeDescendant }: Props = $props();
  let root: HTMLDivElement;
  let sheet: Sheet | undefined;
  /** An input method is composing: the browser owns the page until it ends (03, The editor: Keyboard behaviour). */
  let composing = $state(false);
  /** A pointer is down: the selection is being made, and a drawing would break it. */
  let pressing = false;
  /** The editor is giving itself focus and will place the caret itself. */
  let placing = false;

  function icon(target: HTMLElement, name: CodeIconName): () => void {
    const drawn = mount(CodeIcon, { target, props: { name } });
    return () => void unmount(drawn);
  }

  onMount(() => {
    const made = new Sheet({
      root,
      mode: () => mode,
      chips: () => chips,
      icon,
      now: () => Date.now(),
      changed: (next) => { value = next; },
      moved: (at) => onCaret?.(at),
    });
    sheet = made;
    return () => { made.clear(); };
  });

  // A prompt from outside is taken; a new mode or a new chip reader draws the prompt again (the sheet asks for both).
  $effect(() => { sheet?.load(value); });
  // Not while an input method composes or a pointer selects: the browser owns the selection then, and a drawing would
  // move it. The next change of the selection draws.
  $effect(() => {
    void mode;
    void chips;
    if (composing) return;
    untrack(() => { if (!pressing) sheet?.show(); });
  });

  function hold(): void {
    placing = true;
    root.focus();
    placing = false;
  }

  /** Gives the editor focus, with the caret where it was. */
  export function focus(): void {
    hold();
    sheet?.show();
  }

  /** Replaces a stretch of the Markdown and leaves the caret after the new text: how a mention is put in. */
  export function replace(start: number, end: number, text: string): void {
    if (sheet === undefined) return;
    const from = Math.max(0, Math.min(start, end, sheet.value.length));
    sheet.apply(edit(from, Math.max(from, Math.min(Math.max(start, end), sheet.value.length)), text), 'other');
  }

  /** Puts text at the caret, over the selection if there is one. */
  export function insert(text: string): void {
    if (sheet !== undefined) replace(sheet.picked.start, sheet.picked.end, text);
  }

  function onKeydown(event: KeyboardEvent): void {
    // A key of an input method does nothing here; 229 is the Enter that ends a composition in WebKit.
    if (sheet === undefined || disabled || composing || event.isComposing || event.keyCode === 229) return;
    sheet.heed();
    if (onKey?.(event) === true) {
      event.preventDefault();
      return;
    }
    if (event.defaultPrevented) return;
    const { key, shiftKey, altKey, ctrlKey, metaKey } = event;
    const action = chordAction({ key, shiftKey, altKey, ctrlKey, metaKey, mac: isMac(navigator.userAgent) }, sheet.desk);
    if (action === null) return;
    event.preventDefault();
    sheet.act(action);
  }

  function onBeforeInput(event: InputEvent): void {
    if (sheet === undefined || composing || event.isComposing || event.inputType.includes('Composition')) return;
    // Every change is made on the Markdown and drawn from it, so the browser's own edit never happens.
    event.preventDefault();
    if (disabled) return;
    sheet.heed();
    const range = typeof event.getTargetRanges === 'function' ? event.getTargetRanges()[0] : undefined;
    const target =
      range === undefined
        ? null
        : sheet.span({ node: range.startContainer, offset: range.startOffset }, { node: range.endContainer, offset: range.endOffset });
    const text = event.data ?? event.dataTransfer?.getData('text/plain') ?? null;
    sheet.act(typedAction({ type: event.inputType, text, target }, sheet.desk));
  }

  /** An input the editor did not make itself, as the end of a composition: the page is read back. */
  function onInput(event: Event): void {
    if (!composing && !(event instanceof InputEvent && event.isComposing)) sheet?.absorb();
  }

  function onCompositionEnd(): void {
    composing = false;
    sheet?.absorb();
  }

  function put(text: string): void {
    const clean = text.replaceAll('\r\n', '\n').replaceAll('\r', '\n');
    if (sheet === undefined || clean === '') return;
    const over = sheet.picked;
    if (onLongPaste !== undefined && pasteIsLong(clean, sheet.value.length - (over.end - over.start))) onLongPaste(clean);
    else sheet.apply(edit(over.start, over.end, clean), 'other');
  }

  function onPaste(event: ClipboardEvent): void {
    event.preventDefault();
    if (sheet === undefined || disabled) return;
    sheet.heed();
    const files = Array.from(event.clipboardData?.files ?? []);
    const text = event.clipboardData?.getData('text/plain') ?? '';
    // Cells of a spreadsheet come as a picture and as text: where the picture is not taken, the text is not lost.
    if (files.length === 0 || onFiles?.(files, 'paste') !== true) put(text);
  }

  /** Copy and cut write the selection's Markdown as plain text (03, The editor: Markdown round trip). */
  function onCopy(event: ClipboardEvent): void {
    if (sheet === undefined) return;
    sheet.heed();
    const { start, end } = sheet.picked;
    if (start === end) return;
    event.preventDefault();
    event.clipboardData?.setData('text/plain', sheet.value.slice(start, end));
    if (event.type === 'cut' && !disabled) sheet.apply(edit(start, end, ''), 'other');
  }

  function onDrop(event: DragEvent): void {
    event.preventDefault();
    if (sheet === undefined || disabled) return;
    const files = Array.from(event.dataTransfer?.files ?? []);
    if (files.length > 0) {
      onFiles?.(files, 'drop');
      return;
    }
    const under = offsetUnder(root, event.clientX, event.clientY);
    if (under !== null) sheet.place(under);
    hold();
    put(event.dataTransfer?.getData('text/plain') ?? '');
  }

  /** A press on a chip selects it whole; a press on a task's checkbox leaves the caret where it is. */
  function onMouseDown(event: MouseEvent): void {
    const target = event.target instanceof Element ? event.target : null;
    const chip = target?.closest('.ce-chip') ?? null;
    if (chip === null && target?.closest('.ce-check') == null) return;
    event.preventDefault();
    const span = chip === null ? null : (sheet?.around(chip) ?? null);
    if (span === null || sheet === undefined) return;
    hold();
    sheet.place(span.start, span.end);
    sheet.show();
  }

  function onClick(event: MouseEvent): void {
    const box = event.target instanceof Element ? event.target.closest('.ce-check') : null;
    const line = box?.parentElement ?? null;
    if (line === null || sheet === undefined) return;
    // The box is drawn from the Markdown, so its own change is refused and the Markdown is changed.
    event.preventDefault();
    const at = sheet.around(line);
    const change = disabled || at === null ? null : ticked(sheet.value, at.start, sheet);
    if (change !== null) sheet.apply(change, 'other');
  }

  function onRelease(): void {
    if (!pressing) return;
    pressing = false;
    sheet?.follow(true);
  }
</script>

<svelte:document
  onselectionchange={() => { if (!composing && !pressing) sheet?.follow(); }}
  onpointerup={onRelease}
  onpointercancel={onRelease}
/>

<!-- The editor (03, The editor: Element tree): the text box, and the placeholder over it while the prompt is empty. -->
<div class="code-editor">
  <div
    class="code-editor-page"
    role="textbox"
    tabindex="0"
    aria-multiline="true"
    aria-label={label}
    aria-placeholder={placeholder}
    aria-readonly={disabled ? 'true' : undefined}
    contenteditable={!disabled}
    data-mode={mode}
    bind:this={root}
    onkeydown={onKeydown}
    onbeforeinput={onBeforeInput}
    oninput={onInput}
    oncompositionstart={() => { composing = true; }}
    oncompositionend={onCompositionEnd}
    onpaste={onPaste}
    oncopy={onCopy}
    oncut={onCopy}
    ondrop={onDrop}
    ondragstart={(event) => { event.preventDefault(); }}
    onpointerdown={(event) => { if (event.button === 0) pressing = true; }}
    oncontextmenu={() => { pressing = false; }}
    aria-autocomplete={controls === undefined ? undefined : 'list'}
    aria-controls={controls}
    aria-activedescendant={activeDescendant}
    onmousedown={onMouseDown}
    onclick={onClick}
    onfocus={() => { if (!placing && !pressing) sheet?.follow(true); }}
    onblur={() => { pressing = false; sheet?.follow(true); }}
  ></div>
  {#if value === '' && !composing}
    <div class="code-editor-placeholder" aria-hidden="true">{placeholder}</div>
  {/if}
</div>
