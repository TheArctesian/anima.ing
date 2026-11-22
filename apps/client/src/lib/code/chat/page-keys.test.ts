// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { pageKey } from './page-keys';

afterEach(() => {
  document.body.innerHTML = '';
});

/** A key pressed on `target`, after the part that holds it did or did not take it. */
function press(target: Element, key: string, init: KeyboardEventInit & { taken?: boolean } = {}): KeyboardEvent {
  const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init });
  if (init.taken === true) event.preventDefault();
  Object.defineProperty(event, 'target', { value: target });
  return event;
}

function place(html: string): HTMLElement {
  document.body.innerHTML = html;
  return document.querySelector<HTMLElement>('[data-at]') ?? expect.fail('no [data-at]');
}

describe('09 finding 1 (High): keys inside menus, pickers and dialogs are not the page’s', () => {
  it('moves to the next thread on Down from a plain button', () => {
    expect(pageKey(press(place('<button data-at>Row</button>'), 'ArrowDown'), true, document)).toEqual({ kind: 'move', by: 1 });
  });

  it('leaves Down to a kit menu, a listbox, a radio group and a dialog', () => {
    for (const role of ['menu', 'listbox', 'radiogroup', 'dialog']) {
      const item = place(`<div role="${role}"><button data-at>Choice</button></div>`);
      expect(pageKey(press(item, 'ArrowDown'), true, document), role).toBeNull();
    }
  });

  it('leaves a key that a part already took, such as Escape closing the @ list', () => {
    // jsdom has no contenteditable; a text area is a field where the person types too.
    const editor = place('<textarea data-at></textarea>');
    expect(pageKey(press(editor, 'Escape'), true, document)).toEqual({ kind: 'leave' });
    expect(pageKey(press(editor, 'Escape', { taken: true }), true, document)).toBeNull();
  });

  it('takes no key while a modal is open over the page, not even ⌘N', () => {
    const row = place('<button data-at>Row</button><div role="dialog" aria-modal="true"></div>');
    expect(pageKey(press(row, 'ArrowDown'), true, document)).toBeNull();
    expect(pageKey(press(row, 'n', { metaKey: true }), true, document)).toBeNull();
  });

  it('still takes ⌘⇧] from inside a part, so the thread can change from a scroll area', () => {
    const pane = place('<div data-own-keys><p data-at>Text</p></div>');
    expect(pageKey(press(pane, 'ArrowDown'), true, document)).toBeNull();
    expect(pageKey(press(pane, ']', { metaKey: true, shiftKey: true }), true, document)).toEqual({ kind: 'move', by: 1 });
  });
});

describe('09 finding 16: the modifier is ⌘ on a Mac and Ctrl elsewhere', () => {
  it('leaves Ctrl+N to a Mac text field and takes it elsewhere', () => {
    const field = place('<textarea data-at></textarea>');
    expect(pageKey(press(field, 'n', { ctrlKey: true }), true, document)).toBeNull();
    expect(pageKey(press(field, 'n', { metaKey: true }), true, document)).toEqual({ kind: 'new' });
    expect(pageKey(press(field, 'n', { ctrlKey: true }), false, document)).toEqual({ kind: 'new' });
  });
});
