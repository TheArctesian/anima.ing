/** The sign-in's link invite and one-time token claim, sealed: only `openAccess` reads them, a screen or log sees no fields (R-129). */
export interface HubAccess {
  readonly kind: 'hub-access';
}

/** The parts of a `HubAccess`. Secrets: never shown, logged or kept after the join (LOG-007). */
export interface AccessParts {
  readonly invite: string | null;
  /** The claim of `claimDeviceToken`: it works once, for the device that joined with `invite`. */
  readonly claim: string;
}

const sealed = new WeakMap<HubAccess, AccessParts>();

export function sealAccess(parts: AccessParts): HubAccess {
  const access: HubAccess = Object.freeze({ kind: 'hub-access' });
  sealed.set(access, parts);
  return access;
}

export function openAccess(access: HubAccess): AccessParts | null {
  return sealed.get(access) ?? null;
}
