import { isTauri } from '@tauri-apps/api/core';
import { browserCapabilities } from '$lib/platform/browser';
import type { Capabilities } from '$lib/platform/capabilities';
import { isMac } from '$lib/platform/mac';
import { isApple, isMobile, type Agent } from '$lib/platform/mobile';

/** MOB-002, MOB-004, MOB-031; a phone has no updater (UPD-001), and its copy of recent data is the mobile plan's (IOS-003). */
const MOBILE_SHELL: Capabilities = { ...browserCapabilities, keyStore: true, deviceLink: true, cloudAccount: true };

/** `null` outside the shell (WP-42 D-4). System Settings at Sharing exists on a Mac only (R-059, R-079). */
export function shellCapabilities(agent: Agent = navigator): Capabilities | null {
  if (!isTauri()) return null;
  // The iPhone app shows the sign-in page of the account inside the app (mobile plan M3-5) and plays haptics (M4-4).
  if (isMobile(agent)) {
    const apple = isApple(agent);
    return { ...MOBILE_SHELL, accountSession: apple, haptics: apple, runnerReport: true };
  }
  const { userAgent } = agent;
  return {
    keyStore: true,
    webViewTabs: true,
    popOut: false,
    updates: true,
    processLog: true,
    thisMachine: true,
    systemSettings: isMac(userAgent),
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
    haptics: false,
    overlayTitleBar: isMac(userAgent),
    // On Windows and Linux the window has no frame of the system (tauri.windows.conf.json, tauri.linux.conf.json).
    framelessWindow: !isMac(userAgent),
    recentCopy: true,
  };
}
