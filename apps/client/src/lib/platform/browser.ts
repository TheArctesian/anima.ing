import { parseHubAddress, type HubAddress } from '$lib/hub/hub';
import type { Capabilities } from './capabilities';

/** None of the desktop-only features (CLI-001, UPD-001). */
export const browserCapabilities: Capabilities = {
  keyStore: false,
  webViewTabs: false,
  popOut: false,
  updates: false,
  processLog: false,
  thisMachine: false,
  systemSettings: false,
  builds: false,
  mcpPermissions: false,
  agentThreads: false,
  commandLine: false,
  stayAwake: false,
  localHub: false,
  deviceLink: false,
  cloudAccount: false,
  accountSession: false,
  runnerReport: false,
  overlayTitleBar: false,
  framelessWindow: false,
  recentCopy: false,
};

/** Hosts a plain `http:` page may come from: this machine only, for a local build or test (E-2). */
const LOOPBACK_HOSTS: readonly string[] = ['localhost', '127.0.0.1', '[::1]'];

/**
 * The hub is the origin that served the page (CLI-002, E-2). Any other origin answers `null`: the hub serves the web
 * client only over TLS (SEC-001), and a sign-in sent there would put the password on the network in clear text (CLI-003).
 */
export function servedHubAddress(origin: string): HubAddress | null {
  if (!URL.canParse(origin)) return null;
  const url = new URL(origin);
  if (url.protocol === 'https:') {
    const parsed = parseHubAddress(url.origin);
    return parsed.ok ? parsed.value : null;
  }
  if (url.protocol !== 'http:' || !LOOPBACK_HOSTS.includes(url.hostname)) return null;
  return Object.freeze({ origin: url.origin, http: `${url.origin}/graphql`, socket: `ws://${url.host}/graphql` });
}
