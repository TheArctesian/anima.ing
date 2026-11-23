/**
 * Some Android keyboards move the caret to the start of the box, which breaks xterm.js's before/after text comparison;
 * an empty box avoids it, emptied after xterm.js's own read queued at key-down (mobile plan M4-1).
 */
export function clearAfterTyping(box: HTMLTextAreaElement): void {
  box.addEventListener('input', (event) => {
    if (!(event instanceof InputEvent) || event.isComposing || event.inputType !== 'insertText') return;
    setTimeout(() => {
      box.value = '';
    }, 0);
  });
}
