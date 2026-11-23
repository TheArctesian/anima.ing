import { FitAddon } from '@xterm/addon-fit';
import { SearchAddon } from '@xterm/addon-search';
import { Unicode11Addon } from '@xterm/addon-unicode11';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { WebglAddon } from '@xterm/addon-webgl';
import { Terminal } from '@xterm/xterm';
import '@xterm/xterm/css/xterm.css';
import type { SessionTransfer } from '../api';
import type { CellSize } from '../tunnel/terminal/codec';
import { outputFlow } from './flow';
import { guardEscapes, type EscapeHooks } from './remote/escapes';
import { guardClipboard } from './operator/clipboard';
import { safePaste } from './operator/paste';
import { shortcutOf, type ShortcutAction } from './operator/shortcuts';
import { clearAfterTyping } from './operator/typing';
import { terminalFont, terminalOptions } from './options';

export interface ScreenHooks extends EscapeHooks {
  readonly oninput: (bytes: Uint8Array<ArrayBuffer>) => void;
  readonly onresize: (size: CellSize) => void;
  readonly oncopy: (text: string) => void;
  readonly onfind: () => void;
  /** Output arrived faster than it could be drawn, so some is skipped (once per episode). */
  readonly onoverflow: () => void;
  /** Whether the session lets text be copied out and pasted in now (RA-033); asked at each copy and paste. */
  readonly allows: () => SessionTransfer;
}

/** The only surface over xterm.js outside this folder. */
export interface TerminalScreen {
  size(): CellSize;
  write(bytes: Uint8Array): void;
  reset(): void;
  fit(): void;
  focus(): void;
  blur(): void;
  /** Paste `text` as the operator's own action; bracketed when the shell asked for it, and nothing while paste is off. */
  paste(text: string): void;
  find(text: string, backwards: boolean, incremental: boolean): boolean;
  clearFind(): void;
  dispose(): void;
}

export type ScreenOpener = (host: HTMLElement, hooks: ScreenHooks) => Promise<TerminalScreen>;

/** The first cell measure needs the terminal font loaded. */
async function fontReady(family: string, sizePx: number): Promise<void> {
  if (!('fonts' in document)) return;
  try {
    await document.fonts.load(`${String(sizePx)}px ${family}`);
  } catch {
    // A font that cannot load leaves the fallback family in use; the terminal still works.
  }
}

/** The addon reports a lost context only after a 3 s wait, so the canvas's non-bubbling `webglcontextlost` falls back at once. */
function enableWebgl(terminal: Terminal): void {
  try {
    const webgl = new WebglAddon();
    let disposed = false;
    const fallBack = () => {
      if (disposed) return;
      disposed = true;
      webgl.dispose();
    };
    webgl.onContextLoss(fallBack);
    terminal.loadAddon(webgl);
    terminal.element?.addEventListener('webglcontextlost', fallBack, { capture: true, once: true });
  } catch {
    // No WebGL 2: the DOM renderer stays (xterm.js 6 has no canvas renderer).
  }
}

function acts(terminal: Terminal, action: ShortcutAction, hooks: ScreenHooks): void {
  if (action === 'copy' && hooks.allows().copy && terminal.hasSelection()) hooks.oncopy(terminal.getSelection());
  if (action === 'find') hooks.onfind();
}

function wireEvents(terminal: Terminal, hooks: ScreenHooks): void {
  const encoder = new TextEncoder();
  terminal.onData((text) => {
    hooks.oninput(encoder.encode(text));
  });
  // `onBinary` carries a binary string: one byte per character (mouse reports above column 223).
  terminal.onBinary((text) => {
    hooks.oninput(Uint8Array.from(text, (character) => character.charCodeAt(0)));
  });
  terminal.onResize(({ cols, rows }) => {
    hooks.onresize({ cols, rows });
  });
  terminal.attachCustomKeyEventHandler((event) => {
    const action = shortcutOf(event);
    if (action === undefined) return true;
    if (event.type === 'keydown') {
      // Paste is left to the browser's `paste` event, which xterm.js handles; the others are ours.
      if (action !== 'paste') event.preventDefault();
      acts(terminal, action, hooks);
    }
    return false;
  });
}

/** The session terminal (RA-003, RA-011), with the escapes guarded as K4 §8.7 says. */
export const openScreen: ScreenOpener = async (host, hooks) => {
  const font = terminalFont(host);
  await fontReady(font.fontFamily, font.fontSize);
  // Unicode 11 widths are set through the proposed `unicode` API.
  const terminal = new Terminal({ ...terminalOptions(host, font), allowProposedApi: true });
  const fit = new FitAddon();
  const search = new SearchAddon();
  terminal.loadAddon(fit);
  terminal.loadAddon(search);
  terminal.loadAddon(new Unicode11Addon());
  terminal.unicode.activeVersion = '11';
  terminal.loadAddon(
    new WebLinksAddon((_event, address) => {
      hooks.onlink(address);
    }),
  );
  guardEscapes(terminal, hooks);
  wireEvents(terminal, hooks);
  terminal.open(host);
  // The terminal's own box takes its background, so room inside it (the phone layout's margin) is the terminal's too.
  if (terminal.element !== undefined) terminal.element.style.backgroundColor = terminal.options.theme?.background ?? '';
  if (terminal.textarea !== undefined) clearAfterTyping(terminal.textarea);
  guardClipboard(host, terminal, hooks.allows);
  enableWebgl(terminal);
  fit.fit();
  return screenOf(terminal, fit, search, hooks);
};

function screenOf(terminal: Terminal, fit: FitAddon, search: SearchAddon, hooks: ScreenHooks): TerminalScreen {
  const flow = outputFlow();
  return {
    size: () => ({ cols: terminal.cols, rows: terminal.rows }),
    write: (bytes) => {
      const admission = flow.admit(bytes.length);
      if (admission !== 'draw') {
        if (admission === 'skip-and-tell') hooks.onoverflow();
        return;
      }
      try {
        terminal.write(bytes, () => {
          flow.drawn(bytes.length);
        });
      } catch {
        // xterm.js refuses a write once its own limit is reached: the output is lost, the session is not (K4 §8.5).
        flow.drawn(bytes.length);
      }
    },
    reset: () => {
      terminal.reset();
    },
    fit: () => {
      fit.fit();
    },
    focus: () => {
      terminal.focus();
    },
    blur: () => {
      terminal.blur();
    },
    paste: (text) => {
      if (hooks.allows().paste) terminal.paste(safePaste(text));
    },
    find: (text, backwards, incremental) => {
      const options = { incremental };
      return backwards ? search.findPrevious(text, options) : search.findNext(text, options);
    },
    clearFind: () => {
      search.clearDecorations();
      terminal.clearSelection();
    },
    dispose: () => {
      terminal.dispose();
    },
  };
}
