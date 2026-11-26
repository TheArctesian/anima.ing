import { invoke } from '@tauri-apps/api/core';
import type { Handoff } from './browser';

export function openMobilePreview(endpoint: string, value: Handoff, report: (message: string) => void): () => void {
  const id = crypto.randomUUID();
  let stopped = false;
  const close = () => { void invoke('mobile_preview_close', { id }).catch(() => {}); };
  if (value.expires <= Date.now()) { report('Prepare a fresh preview link.'); return () => {}; }
  void invoke('mobile_preview_open', { id, endpoint, action: value.action, token: value.token })
    .then(() => { if (stopped) close(); })
    .catch(() => { if (!stopped) report('The preview could not open. Close any existing preview and prepare it again.'); });
  return () => { stopped = true; close(); };
}
