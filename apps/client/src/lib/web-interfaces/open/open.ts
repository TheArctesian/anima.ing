import type { OpenIn, OperatingSystem, Status } from '$lib/hub/generated/graphql';
import type { WebViewShell } from '$lib/web-interfaces/shell/shell';
import type { Sessions } from '$lib/sessions/sessions.svelte';
import { capabilities } from '$lib/platform/platform';
import type { Origin, WebTarget } from '$lib/sessions/tabs';

export interface WebOpening {
  readonly device: { readonly id: string; readonly name: string; readonly status: Status; readonly os?: OperatingSystem; readonly distribution?: string | null };
  readonly webInterface: WebTarget & { readonly openIn: OpenIn };
  readonly origin: Origin | null;
}

/** An `http` interface in the system browser needs no tab: there is no pin to read (WEB-011). */
function belongsToBrowser({ webInterface }: WebOpening): boolean {
  return webInterface.openIn === 'SYSTEM_BROWSER' && URL.canParse(webInterface.url) && new URL(webInterface.url).protocol === 'http:';
}

/** Only an `http:` or `https:` address opens; any other scheme, or text that is no URL, opens nothing (WEB-011). */
function openInBrowserTab(url: string): void {
  const parsed = URL.parse(url);
  if (parsed === null || (parsed.protocol !== 'http:' && parsed.protocol !== 'https:')) return;
  // `noopener` makes `window.open` return null even when the tab opened. The URL is not logged (LOG-007).
  // The parsed address opens, not the text, so `https:evil` opens the host `evil` that was checked.
  window.open(parsed.href, '_blank', 'noopener,noreferrer');
}

/**
 * In a browser the tab opens before any `await`, inside the click, so a popup blocker sees the gesture (WEB-011).
 * `SYSTEM_BROWSER` over `https` opens a web tab that compares the pinned fingerprint first (WEB-003, WEB-008).
 */
export async function openWebInterface(sessions: Pick<Sessions, 'openWeb'>, shell: Pick<WebViewShell, 'openSystemBrowser'>, opening: WebOpening): Promise<void> {
  if (!capabilities().webViewTabs) {
    openInBrowserTab(opening.webInterface.url);
    return;
  }
  if (belongsToBrowser(opening)) {
    try {
      await shell.openSystemBrowser(opening.webInterface.url);
      return;
    } catch {
      // The tab says what is wrong; the shell's own message is not shown (LOG-007).
    }
  }
  const { device, webInterface, origin } = opening;
  const { id, deviceId, url, label, pinnedFingerprintSha256 } = webInterface;
  sessions.openWeb({ deviceId: device.id, deviceName: device.name, ...(device.os === undefined ? {} : { os: device.os }), ...(device.distribution == null ? {} : { distribution: device.distribution }), status: device.status, web: { id, deviceId, url, label, pinnedFingerprintSha256 }, origin });
}
