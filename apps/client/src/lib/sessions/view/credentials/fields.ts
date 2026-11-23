import type { RemoteProtocol } from '../../api';

/** A connection parameter the gateway may ask the operator for (GW-015, K4 §4). */
export type CredentialName = 'username' | 'domain' | 'password';

export interface CredentialField {
  readonly name: CredentialName;
  readonly label: string;
  readonly secret: boolean;
}

/** The names the hub relays in `argv` for each protocol, in prompt order (K4 §4). */
const ALLOWED: Readonly<Record<RemoteProtocol, readonly CredentialName[]>> = {
  RDP: ['username', 'domain', 'password'],
  VNC: ['username', 'password'],
};

const FIELDS: Readonly<Record<CredentialName, CredentialField>> = {
  username: { name: 'username', label: 'Username', secret: false },
  domain: { name: 'domain', label: 'Domain', secret: false },
  password: { name: 'password', label: 'Password', secret: true },
};

/**
 * Undefined, and the caller ends the session, when `required` is empty or names a parameter outside the protocol's set:
 * the gateway cannot ask the operator for anything else, and the hub drops `argv` for other names (fail closed).
 */
export function askedFields(protocol: RemoteProtocol, required: readonly string[]): readonly CredentialField[] | undefined {
  const allowed: readonly string[] = ALLOWED[protocol];
  if (required.length === 0 || required.some((name) => !allowed.includes(name))) return undefined;
  return ALLOWED[protocol].filter((name) => required.includes(name)).map((name) => FIELDS[name]);
}
