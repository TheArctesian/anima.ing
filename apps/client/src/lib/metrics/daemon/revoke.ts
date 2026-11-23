const STOPS = 'The daemon stops sending metrics. The device keeps the Daemon source, shown as stale, until you change it.';

/** A hosted hub has no VPN and no Headscale, so its warning names neither (MET-013, R-105, R-222). */
export function revokeWarning(hosted: boolean): string {
  return hosted ? STOPS : `${STOPS} VPN access stays: for a device on the VPN only for the daemon, also delete its node in Headscale.`;
}
