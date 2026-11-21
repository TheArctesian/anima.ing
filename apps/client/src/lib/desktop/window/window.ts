import { getCurrentWindow } from '@tauri-apps/api/window';

/**
 * The window of the application, for a window that the system gives no frame (task TC2, "Both together"). The
 * client draws the three controls that the frame had; these are the commands behind them. Each is sent for the window
 * of this web view. The command of Tauri takes the label of a window, so a script of the main web view could name a
 * different window of the application; the capability gives it to the main web view only.
 */
export interface WindowFrame {
  minimise(): Promise<void>;
  /** Fills the screen, or goes back to the size before. */
  toggleMaximise(): Promise<void>;
  close(): Promise<void>;
  isMaximised(): Promise<boolean>;
  /** Calls `listener` each time the size of the window changes; the answer stops it. */
  onResized(listener: () => void): Promise<() => void>;
}

/** The window that this web view is in. Only the desktop shell answers these commands. */
export function shellWindow(): WindowFrame {
  const current = getCurrentWindow();
  return {
    minimise: () => current.minimize(),
    toggleMaximise: () => current.toggleMaximize(),
    close: () => current.close(),
    isMaximised: () => current.isMaximized(),
    onResized: (listener) => current.onResized(listener),
  };
}
