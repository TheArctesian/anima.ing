import { startMobilePreview, stopMobilePreview } from '$lib/desktop/web-view/workspace';
import type { Handoff } from './browser';

export function openMobilePreview(endpoint: string, value: Handoff, report: (message: string) => void): () => void {
  const id = crypto.randomUUID();
  let stopped = false;
  const close = () => { void stopMobilePreview(id).catch(() => undefined); };
  if (value.expires <= Date.now()) { report('Prepare a fresh preview link.'); return () => undefined; }
  void startMobilePreview(id, { endpoint, action: value.action, token: value.token })
    .then(() => { if (stopped) close(); })
    .catch(() => { if (!stopped) report('The preview could not open. Close any existing preview and prepare it again.'); });
  return () => { stopped = true; close(); };
}
