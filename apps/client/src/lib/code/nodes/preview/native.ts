import { sendPreviewHandoff } from '$lib/desktop/web-view/workspace';
import { desktopWebView } from '$lib/desktop/web-view/web-view';
import type { Handoff } from './browser';

/** Reuses the capability-free device view; the main app alone transfers its handoff. */
export function openNativePreview(device: string, endpoint: string, value: Handoff, report: (message: string) => void): () => void {
  const tabId = `code-preview-${crypto.randomUUID()}`;
  const launch = `${new URL(value.action).origin}/.anima/connect`;
  let stopped = false;
  let opened = false;
  let unlisten: (() => void) | undefined;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let cancelLoad: (() => void) | undefined;
  const stop = () => {
    stopped = true;
    clearTimeout(timer); unlisten?.(); cancelLoad?.();
    if (opened) void desktopWebView.close(tabId).catch(() => undefined);
  };
  void (async () => {
    try {
      if (value.expires <= Date.now()) throw new Error();
      let loaded!: () => void;
      let failed!: () => void;
      const page = new Promise<void>((resolve, reject) => { loaded = resolve; failed = () => reject(new Error()); });
      // Observe rejection immediately, including a page failure while open() still awaits.
      void page.catch(() => undefined);
      cancelLoad = failed;
      timer = setTimeout(failed, 15_000);
      unlisten = await desktopWebView.listen((event) => {
        if (event.tabId !== tabId) return;
        if (event.kind === 'LOADED' && event.url === launch) loaded();
        else if (event.kind === 'FAILED' || event.kind === 'UNTRUSTED' || event.kind === 'MISMATCH') failed();
      });
      if (stopped) { unlisten(); return; }
      await desktopWebView.open({ tabId, deviceId: device, url: launch, pin: null, previewEndpoint: endpoint, bounds: { x: 0, y: 0, width: 1, height: 1 } });
      opened = true;
      if (stopped) { await desktopWebView.close(tabId); return; }
      await desktopWebView.popOut(tabId);
      await page;
      clearTimeout(timer); unlisten(); unlisten = undefined; cancelLoad = undefined;
      if (stopped || value.expires <= Date.now()) throw new Error();
      await sendPreviewHandoff(tabId, { endpoint, action: value.action, token: value.token });
    } catch {
      if (!stopped) report('The private preview could not open in this app. Check its connection and certificate, then prepare it again.');
      stop();
    }
  })();
  return stop;
}
