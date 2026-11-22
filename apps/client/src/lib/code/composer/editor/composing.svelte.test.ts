// @vitest-environment jsdom
import { flushSync, mount, unmount, type ComponentProps } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import Editor from './Editor.svelte';
import { pointAt } from './view/points';

type Props = ComponentProps<typeof Editor>;

let app: ReturnType<typeof mount> | undefined;

afterEach(() => {
  if (app) void unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

/** The editor in a page with focus, its prompt bound to `props.value` and the caret at `caret`. */
function editor(start: Partial<Props>, caret: number) {
  const props: Props = $state({ value: '', placeholder: 'Ask anything', label: 'Message', ...start });
  app = mount(Editor, { target: document.body, props });
  flushSync();
  const box = document.querySelector<HTMLElement>('[role="textbox"]');
  if (box === null) throw new Error('no text box');
  box.focus();
  const at = pointAt(box, caret);
  document.getSelection()?.setBaseAndExtent(at.node, at.offset, at.node, at.offset);
  document.dispatchEvent(new Event('selectionchange'));
  flushSync();
  return { props, box };
}

function press(box: HTMLElement, key: string, more: KeyboardEventInit = {}): KeyboardEvent {
  const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...more });
  box.dispatchEvent(event);
  flushSync();
  return event;
}

function composition(box: HTMLElement, type: 'compositionstart' | 'compositionend', data = ''): void {
  box.dispatchEvent(new CompositionEvent(type, { bubbles: true, data }));
  flushSync();
}

describe('an input method', () => {
  it('does nothing while an input method composes, then reads its text from the page', () => {
    const onKey = vi.fn(() => false);
    const { box, props } = editor({ value: '- ', onKey }, 2);
    composition(box, 'compositionstart');
    expect(press(box, 'Enter', { isComposing: true }).defaultPrevented).toBe(false);
    expect(press(box, 'Process', { keyCode: 229 }).defaultPrevented).toBe(false);
    const typed = new InputEvent('beforeinput', { inputType: 'insertCompositionText', data: 'か', isComposing: true, bubbles: true, cancelable: true });
    box.dispatchEvent(typed);
    expect(typed.defaultPrevented).toBe(false);
    expect(onKey).not.toHaveBeenCalled();
    const caret = document.getSelection();
    caret?.anchorNode?.insertBefore(document.createTextNode('漢字'), caret.anchorNode.childNodes[caret.anchorOffset] ?? null);
    composition(box, 'compositionend', '漢字');
    expect(props.value).toBe('- 漢字');
    // WebKit sends the Enter that ended the composition after its end, with code 229: it is not the composer's.
    expect(press(box, 'Enter', { keyCode: 229 }).defaultPrevented).toBe(false);
    expect(onKey).not.toHaveBeenCalled();
    press(box, 'z', { ctrlKey: true });
    expect(props.value).toBe('- ');
  });

  it('does not draw a prompt that comes from outside while an input method composes, and keeps the composed words', () => {
    const { box, props } = editor({ value: 'sent' }, 4);
    composition(box, 'compositionstart');
    const composed = document.createTextNode('かな');
    box.firstChild?.appendChild(composed);
    props.value = '';
    flushSync();
    expect(composed.isConnected).toBe(true);
    composition(box, 'compositionend', '仮名');
    expect(props.value).toBe('仮名');
    expect(box.textContent).toBe('仮名');
  });

  it('draws the page anew after a composition that was cancelled', () => {
    const { box, props } = editor({}, 0);
    composition(box, 'compositionstart');
    box.querySelector('.ce-line')?.replaceChildren();
    composition(box, 'compositionend');
    expect(props.value).toBe('');
    expect(box.querySelector('.ce-line > br')).not.toBeNull();
  });
});

describe('the way out of where Tab indents', () => {
  it('leaves the Tab after Escape in a code block to the browser, so the keyboard can leave the editor', () => {
    const { box, props } = editor({ value: '```\ncode\n```' }, 8);
    expect(press(box, 'Tab').defaultPrevented).toBe(true);
    expect(props.value).toBe('```\ncode  \n```');
    press(box, 'Escape');
    press(box, 'Shift', { shiftKey: true });
    expect(press(box, 'Tab', { shiftKey: true }).defaultPrevented).toBe(false);
    expect(press(box, 'Tab').defaultPrevented).toBe(true);
    press(box, 'Escape');
    press(box, 'ArrowLeft');
    expect(press(box, 'Tab').defaultPrevented).toBe(true);
    press(box, 'Escape');
    box.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true }));
    expect(press(box, 'Tab').defaultPrevented).toBe(true);
    expect(props.value).toBe(`\`\`\`\ncode${' '.repeat(8)}\n\`\`\``);
  });

  it('leaves the Tab after Escape on a list line to the browser, and says so in the description of the text box', () => {
    const { box, props } = editor({ value: '- a\n- b' }, 7);
    press(box, 'Escape');
    expect(press(box, 'Tab').defaultPrevented).toBe(false);
    expect(press(box, 'Tab').defaultPrevented).toBe(true);
    expect(props.value).toBe('- a\n  - b');
    const described = document.getElementById(box.getAttribute('aria-describedby') ?? '');
    expect(described?.textContent).toBe('In a list or a code block Tab indents. Press Escape, then Tab, to leave the editor.');
  });

  it('does not arm the Tab with an Escape that the composer took', () => {
    const { box } = editor({ value: '```\ncode\n```', onKey: (event) => event.key === 'Escape' }, 8);
    press(box, 'Escape');
    expect(press(box, 'Tab').defaultPrevented).toBe(true);
  });
});
