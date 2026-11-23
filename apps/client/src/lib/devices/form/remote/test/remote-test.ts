import type { ConnectionTestInput, Protocol } from '$lib/hub/generated/graphql';
import type { ConnectionTest, ServerCertificate } from '$lib/sessions/api';
import type { ConnectionTestResult } from '../../connection-test';
import { normalizedValues, type DeviceFormValues } from '../../values';
import { rdpCredentialInput, rdpInput, vncCredentialInput, vncInput } from '../input';

export type RemoteProtocol = Exclude<Protocol, 'SSH'>;

/** The connection test input with the K1 additions (DEV-013). */
export type RemoteTestInput = ConnectionTestInput;

export type RemoteTestResult = ConnectionTestResult & Partial<Pick<ConnectionTest, 'serverCertificate' | 'certificateChanged'>>;

function sameJson(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/** Whether the saved device's host and `protocol` settings are as saved, so that the test can run against the stored record. */
export function testsSavedRemote(
  protocol: RemoteProtocol,
  initial: DeviceFormValues | null,
  values: DeviceFormValues,
  deviceId: string | null,
): initial is DeviceFormValues {
  if (initial === null || deviceId === null) return false;
  const before = normalizedValues(initial);
  const now = normalizedValues(values);
  if (before.host !== now.host) return false;
  return protocol === 'RDP'
    ? sameJson(rdpInput(before.rdp), rdpInput(now.rdp, before.rdp.port))
    : sameJson(vncInput(before.vnc), vncInput(now.vnc, before.vnc.port));
}

/**
 * DEV-013: an unchanged saved device is tested by ID, else by values, since the hub refuses both together. The SSH
 * credential is never part of it, nor is the other protocol's password.
 */
export function remoteTestInput(
  protocol: RemoteProtocol,
  initial: DeviceFormValues | null,
  values: DeviceFormValues,
  deviceId: string | null,
): RemoteTestInput {
  const password = protocol === 'RDP' ? rdpCredentialInput(values) : vncCredentialInput(values);
  const extra = password === null ? {} : { password };
  if (testsSavedRemote(protocol, initial, values, deviceId) && deviceId !== null) return { deviceId, protocol, ...extra };
  const now = normalizedValues(values);
  const before = initial === null ? undefined : normalizedValues(initial);
  const settings =
    protocol === 'RDP' ? { rdp: rdpInput(now.rdp, before?.rdp.port) } : { vnc: vncInput(now.vnc, before?.vnc.port) };
  return { host: now.host, protocol, ...settings, ...extra };
}

export type RemoteOutcome =
  | { readonly kind: 'reachable'; readonly message: string }
  | { readonly kind: 'unreachable'; readonly message: string }
  | { readonly kind: 'certificateNew'; readonly certificate: ServerCertificate }
  | { readonly kind: 'certificateChanged'; readonly certificate: ServerCertificate | null };

const UNREACHABLE_MESSAGE = 'The device did not answer.';

/**
 * DEV-013, SEC-007: a changed RDP certificate is never reachable-only. `authenticated` is not read: RDP and VNC sign in
 * only when a session starts. A `null` certificate says nothing about the pin.
 */
export function remoteOutcome(protocol: RemoteProtocol, result: RemoteTestResult, acceptedFingerprint: string | null = null): RemoteOutcome {
  const message = result.message ?? '';
  if (!result.reachable) return { kind: 'unreachable', message: message === '' ? UNREACHABLE_MESSAGE : message };
  if (protocol === 'RDP') {
    const certificate = result.serverCertificate ?? null;
    if (result.certificateChanged === true) return { kind: 'certificateChanged', certificate };
    if (certificate !== null && certificate.fingerprintSha256 !== acceptedFingerprint) return { kind: 'certificateNew', certificate };
  }
  return { kind: 'reachable', message };
}

const REACHABLE_TEXT = 'The device answered. Anima.ing signs in when a session starts.';
const CHANGED_CERTIFICATE_WARNING = 'The server certificate changed. Do not trust this device until you check it.';

export function remoteOutcomeText(outcome: RemoteOutcome): string {
  switch (outcome.kind) {
    case 'reachable':
      // The hub's message is fixed text (R-035).
      return outcome.message === '' ? REACHABLE_TEXT : outcome.message;
    case 'unreachable':
      return outcome.message;
    case 'certificateNew':
      return 'New server certificate. Check the fingerprint before you trust it.';
    case 'certificateChanged':
      return CHANGED_CERTIFICATE_WARNING;
  }
}

export function isRemoteWarning(outcome: RemoteOutcome): boolean {
  return outcome.kind === 'certificateChanged';
}
