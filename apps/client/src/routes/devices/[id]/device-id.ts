/** A device's ID as the hub writes it (K7). */
export type DeviceId = string & { readonly __brand: 'DeviceId' };

const UUID = /^[\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12}$/i;

/** Null for a parameter that is not a device ID, so no request is sent for it (D-14). */
export function parseDeviceId(text: string): DeviceId | null {
  return UUID.test(text) ? (text as DeviceId) : null;
}
