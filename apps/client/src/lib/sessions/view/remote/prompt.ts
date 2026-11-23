import type { RemoteProtocol } from '../../api';
import { askedFields, type CredentialField } from '../credentials/fields';

export type GatewayAsk =
  | { readonly kind: 'none' }
  | { readonly kind: 'ask'; readonly fields: readonly CredentialField[]; readonly required: readonly string[] }
  | { readonly kind: 'refuse' };

const NONE: GatewayAsk = { kind: 'none' };

/** `answered` is the instruction the operator already answered; it asks nothing more. */
export function gatewayAsk(protocol: RemoteProtocol, required: readonly string[] | undefined, answered: readonly string[] | undefined): GatewayAsk {
  if (required === undefined || required === answered) return NONE;
  const fields = askedFields(protocol, required);
  return fields === undefined ? { kind: 'refuse' } : { kind: 'ask', fields, required };
}

/** Fixed texts of a session the prompt ended; never the gateway's names (LOG-007, R-035). */
export const PROMPT_CANCELLED = 'Sign-in cancelled. Session ended.';
export const PROMPT_REFUSED = 'The device asked for a sign-in Anima.ing cannot give. Session ended.';
