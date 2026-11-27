/** One answer per desktop-only feature; every desktop-only control reads it through `$lib/platform/platform` (WP-42 D-4, CLI-001). */
export interface Capabilities {
  /** The OS key store holds the access token (CLI-003). */
  readonly keyStore: boolean;
  /** Web interfaces open in native web view tabs that can pin a certificate (WEB-003, WEB-008). */
  readonly webViewTabs: boolean;
  /** Isolated native mobile workspace preview, without app IPC. */
  readonly workspacePreviews: boolean;
  /** A session tab can move to its own window (RA-002). */
  readonly popOut: boolean;
  /** The app checks for and installs its own updates (UPD-001, UPD-006). */
  readonly updates: boolean;
  /** The app keeps a process log and can open its folder (LOG-008). */
  readonly processLog: boolean;
  /** The shell can read the computer the app runs on, for Add device (R-058, R-060). */
  readonly thisMachine: boolean;
  /** The shell can open System Settings at Sharing, for Remote Login and Screen Sharing (R-059, R-079). */
  readonly systemSettings: boolean;
  /** The shell runs the build steps and keeps the builder token (BLD-020, BLD-026). */
  readonly builds: boolean;
  /** The shell reads and saves the `anima mcp` permissions in the command line's configuration file (MCP-018, MCP-023). */
  readonly mcpPermissions: boolean;
  /** The shell puts the `anima` program the app carries on the `PATH` with a link or launcher (INS-004 to INS-006). */
  readonly commandLine: boolean;
  /** The shell keeps this computer awake and has the tray item for it (AWK-004). */
  readonly stayAwake: boolean;
  /** The shell installs, starts and stops the hub on this computer and joins this device to a hub over the link (FRN-002, FRN-005). */
  readonly localHub: boolean;
  /** The shell joins this device to a hub over the link; the hosted sign-in needs it (FRN-005, MOB-002, R-129, MOB-031). */
  readonly deviceLink: boolean;
  /** The shell signs in to the Anima.ing account (HM-T1 to HM-T7); `cloud_device` reads a computer, so it goes with `thisMachine`. */
  readonly cloudAccount: boolean;
  /** The shell shows the account sign-in page inside the app, with no code to compare (mobile plan M3-5). */
  readonly accountSession: boolean;
  /** The shell reads the sessions of Claude Code and Codex of this computer as threads (THR-011). */
  readonly agentThreads: boolean;
  /** The shell can read this device’s local runner capability report (OD-14). */
  readonly runnerReport: boolean;
  /** The window draws its content under the OS title bar, with the window buttons over the navigation bar (macOS). */
  readonly overlayTitleBar: boolean;
  /** The client draws the window controls and the top of the window moves it (Windows and Linux; agent-threads.md TC2). */
  readonly framelessWindow: boolean;
  /** The app keeps a read-only copy of recent hub answers on this computer and shows it when the hub does not answer (FND-007). */
  readonly recentCopy: boolean;
}
