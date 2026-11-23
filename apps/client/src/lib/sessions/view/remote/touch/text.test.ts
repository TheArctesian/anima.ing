// @vitest-environment jsdom
import { mount, unmount } from 'svelte';
import { expect, it, vi } from 'vitest';
import TouchText from './TouchText.svelte';

it('sends committed text and editing keys without forwarding pasted data', async () => {
  const target = document.createElement('div');
  const sendKeyEvent = vi.fn();
  const view = mount(TouchText, { target, props: { client: { sendKeyEvent } } });
  try {
    const field = target.querySelector('input');
    if (field === null) throw new Error('The remote text field is missing');
    field.value = 'e';
    field.dispatchEvent(new InputEvent('input', { bubbles: true, isComposing: true }));
    expect(sendKeyEvent).not.toHaveBeenCalled();
    field.value = 'é';
    field.dispatchEvent(new InputEvent('input', { bubbles: true }));
    expect(field.value).toBe('');
    field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Backspace', bubbles: true, cancelable: true }));
    const paste = new InputEvent('beforeinput', { inputType: 'insertFromPaste', data: 'clipboard', bubbles: true, cancelable: true });
    expect(field.dispatchEvent(paste)).toBe(false);
    expect(sendKeyEvent.mock.calls).toEqual([[1, 0xe9], [0, 0xe9], [1, 0xff08], [0, 0xff08]]);
  } finally {
    await unmount(view);
  }
});
