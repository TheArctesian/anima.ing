import type { Terminal } from '@xterm/xterm';
import type { SessionTransfer } from '../../api';
import { safePaste } from './paste';

/** Edits that put outside text into xterm.js's hidden text field; paste and drop come through their own events. */
const OUTSIDE_TEXT = new Set(['insertFromPaste', 'insertFromPasteAsQuotation', 'insertFromDrop', 'insertFromYank']);

function refuse(event: Event): void {
  event.preventDefault();
  event.stopPropagation();
}

/**
 * Takes the clipboard's ways in and out before xterm.js sees them (RA-003, RA-033); `allows` is asked at each event.
 * xterm.js selects a mouse or right-clicked selection in its text field, which reaches the primary selection.
 * Call after `terminal.open`, which makes the field.
 */
export function guardClipboard(host: HTMLElement, terminal: Terminal, allows: () => SessionTransfer): void {
  const field = terminal.textarea;
  if (field !== undefined) {
    const select = field.select.bind(field);
    field.select = () => {
      if (allows().copy) select();
      else field.value = '';
    };
  }
  const pasteIn = (text: string) => {
    if (allows().paste) terminal.paste(safePaste(text));
  };
  host.addEventListener(
    'paste',
    (event) => {
      refuse(event);
      pasteIn(event.clipboardData?.getData('text/plain') ?? '');
    },
    true,
  );
  host.addEventListener(
    'drop',
    (event) => {
      refuse(event);
      pasteIn(event.dataTransfer?.getData('text/plain') ?? '');
    },
    true,
  );
  host.addEventListener(
    'beforeinput',
    (event) => {
      if (OUTSIDE_TEXT.has(event.inputType)) refuse(event);
    },
    true,
  );
  for (const type of ['copy', 'cut', 'contextmenu']) {
    host.addEventListener(
      type,
      (event) => {
        if (!allows().copy) refuse(event);
      },
      true,
    );
  }
}
