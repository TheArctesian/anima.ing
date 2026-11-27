import { object } from '../receipts';

export interface Handoff { action: string; token: string; expires: number }
export function handoff(value: unknown, endpoint: string, appOrigin: string): Handoff | null {
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(endpoint)) return null;
  const row = object(value);
  if (!row || row.method !== 'POST' || typeof row.action !== 'string' || typeof row.token !== 'string' || !/^[A-Za-z0-9_-]{43}$/.test(row.token)) return null;
  const action = endpointAction(row.action, endpoint, appOrigin);
  return action ? { action, token: row.token, expires: Date.now() + 55_000 } : null;
}

function endpointAction(address: string, endpoint: string, appOrigin: string): string | null {
  try {
    const url = new URL(address);
    if (url.protocol !== 'https:' || url.username || url.password || url.search || url.hash) return null;
    if (url.pathname !== '/.anima/handoff' || url.origin === appOrigin || url.hostname.split('.')[0] !== endpoint.replaceAll('-', '')) return null;
    return url.href;
  } catch { return null; }
}

/** Called directly from a click; the preview receives only its own one-use credential. */
export function openPreview(value: Handoff, report: (message: string) => void): () => void {
  if (value.expires <= Date.now()) { report('Prepare a fresh preview link.'); return () => undefined; }
  const origin = new URL(value.action).origin;
  let popup: Window | null = null;
  const stop = () => { window.removeEventListener('message', ready); clearTimeout(timer); };
  const ready = (event: MessageEvent) => {
    if (event.source !== popup || event.origin !== origin || object(event.data)?.type !== 'anima-preview-ready') return;
    popup?.postMessage({ type: 'anima-preview-open', token: value.token }, origin);
    stop();
  };
  window.addEventListener('message', ready);
  popup = window.open(`${origin}/.anima/connect`, '_blank');
  const timer = setTimeout(() => { stop(); report('The preview did not open. Prepare it again, or check whether this app allows preview windows.'); }, 15_000);
  if (!popup) { stop(); report('Allow a preview window, then prepare it again.'); }
  return () => { stop(); popup?.close(); };
}
