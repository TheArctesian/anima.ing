import { browserCapabilities } from './browser';
import type { Capabilities } from './capabilities';

// The answers a test mocks `$lib/platform/platform` with.

export const DESKTOP_MAC: Capabilities = {
  keyStore: true,
  webViewTabs: true,
  popOut: false,
  updates: true,
  processLog: true,
  thisMachine: true,
  systemSettings: true,
  builds: true,
  mcpPermissions: true,
  agentThreads: true,
  commandLine: true,
  stayAwake: true,
  localHub: true,
  deviceLink: true,
  cloudAccount: true,
  accountSession: false,
  runnerReport: false,
  overlayTitleBar: true,
  framelessWindow: false,
  recentCopy: true,
};

export const BROWSER: Capabilities = browserCapabilities;

/** The mobile app on Android: the key store, the device link, the account sign-in and no desktop-only feature (MOB-004). */
export const MOBILE: Capabilities = { ...browserCapabilities, keyStore: true, deviceLink: true, cloudAccount: true, runnerReport: true };

/** The mobile app on an iPhone or an iPad: it also signs in inside the app (mobile plan M3-5). */
export const MOBILE_APPLE: Capabilities = { ...MOBILE, accountSession: true };
