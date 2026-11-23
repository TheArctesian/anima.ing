import { getCurrentWindow } from '@tauri-apps/api/window';

/** The controls of a frameless window (TC2); the capability gives these commands to the main web view only. */
export interface WindowFrame {
  minimise(): Promise<void>;
  toggleMaximise(): Promise<void>;
  close(): Promise<void>;
  isMaximised(): Promise<boolean>;
  onResized(listener: () => void): Promise<() => void>;
}

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
