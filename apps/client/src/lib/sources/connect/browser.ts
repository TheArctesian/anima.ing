import { capabilities } from '$lib/platform/platform';
import { webViewShell } from '$lib/web-interfaces/shell/client-shell';

/** Made inside the click: a browser lets a click open a tab, not a later callback. `open` keeps nothing. */
export interface LinkOpening {
  open(link: string): Promise<boolean>;
  close(): void;
}

export function systemBrowserOpening(openSystemBrowser: (url: string) => Promise<void>): LinkOpening {
  return {
    open: async (link) => {
      try {
        await openSystemBrowser(link);
        return true;
      } catch {
        return false;
      }
    },
    close: () => undefined,
  };
}

export function blankTabOpening(openTab: () => Window | null): LinkOpening {
  const tab = openTab();
  if (tab !== null) tab.opener = null;
  return {
    open: (link) => {
      if (tab === null || tab.closed) return Promise.resolve(false);
      tab.location.href = link;
      return Promise.resolve(true);
    },
    close: () => {
      tab?.close();
    },
  };
}

/** Call it inside the click handler. */
export function linkOpening(): LinkOpening {
  if (capabilities().webViewTabs) return systemBrowserOpening((url) => webViewShell().openSystemBrowser(url));
  return blankTabOpening(() => window.open('about:blank', '_blank'));
}
