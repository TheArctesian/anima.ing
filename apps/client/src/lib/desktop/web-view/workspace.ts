import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

interface PreviewGrant {
  readonly endpoint: string;
  readonly action: string;
  readonly token: string;
}

export function sendPreviewHandoff(tabId: string, grant: PreviewGrant): Promise<void> {
  return invoke('web_view_preview_handoff', { tabId, ...grant });
}

export function startMobilePreview(id: string, grant: PreviewGrant): Promise<void> {
  return invoke('mobile_preview_open', { id, ...grant });
}

export function stopMobilePreview(id: string): Promise<void> {
  return invoke('mobile_preview_close', { id });
}

export function startWorkspaceForward(opening: PreviewGrant & { id: string; localPort: number }): Promise<unknown> {
  return invoke('workspace_forward_start', { opening });
}

export function stopWorkspaceForward(id: string): Promise<void> {
  return invoke('workspace_forward_stop', { id });
}

export function listenWorkspaceForwardEnded(ended: (id: string) => void): Promise<() => void> {
  return listen<string>('workspace-forward-ended', ({ payload }) => { ended(payload); });
}
