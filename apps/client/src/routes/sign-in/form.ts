import { TOKEN_UNREADABLE, type FieldError, type HubError } from '$lib/hub/hub';

export type LoginField = 'username' | 'password' | 'totpCode';

export interface LoginForm {
  readonly username: string;
  readonly password: string;
  readonly totpCode: string;
}

export type FieldMessages = Partial<Record<LoginField, string>>;

const TOTP_SHAPE = /^\d{6}$/;

/** An empty code is allowed: an account may have no second factor (OD-2). */
export function checkLogin(form: LoginForm): FieldMessages {
  const problems: FieldMessages = {};
  if (form.username.trim() === '') problems.username = 'Type your user name.';
  if (form.password === '') problems.password = 'Type your password.';
  if (form.totpCode !== '' && !TOTP_SHAPE.test(form.totpCode)) problems.totpCode = 'The code is six digits.';
  return problems;
}

/** The hub names the input fields as the schema does. */
export function fieldMessages(fields: readonly FieldError[]): FieldMessages {
  const messages: FieldMessages = {};
  for (const { field, message } of fields) {
    if (field === 'username' || field === 'password' || field === 'totpCode') messages[field] = message;
  }
  return messages;
}

const REFUSED_DESKTOP = 'The hub refused. Check the VPN client and hub address.';

/**
 * The hub's own text is never shown: sign-in input must not come back in the page (LOG-007).
 * In a browser, FORBIDDEN is a network the hub does not permit (NET-002) or a page at another origin (K7).
 */
export function failureMessage(error: HubError, desktop: boolean): string {
  switch (error.code) {
    case 'UNAUTHENTICATED':
      // The request never left: the key store could not be read, which is not a wrong password.
      if (error === TOKEN_UNREADABLE) return "Can't sign in: can't read the key store.";
      return 'Wrong user name, password or code.';
    case 'VALIDATION':
      return 'Check the marked fields.';
    case 'unreachable':
      return 'The hub did not answer. Check the VPN client.';
    case 'FORBIDDEN':
      return desktop ? REFUSED_DESKTOP : 'The hub refused sign-in from here. Check the VPN client and page address.';
    case 'malformed':
      return desktop ? REFUSED_DESKTOP : 'The hub refused. Check the VPN client.';
    case 'UNAVAILABLE':
      return 'The hub cannot reach its database. Retry shortly.';
    default:
      return 'Sign-in failed. Retry.';
  }
}

const FIELD_ORDER: readonly LoginField[] = ['username', 'password', 'totpCode'];

export function firstInvalid(fields: FieldMessages): LoginField | undefined {
  return FIELD_ORDER.find((field) => fields[field] !== undefined);
}

export function afterEdit(fields: FieldMessages, field: LoginField, value: string): FieldMessages {
  const probe: LoginForm = { username: 'x', password: 'x', totpCode: '', [field]: value };
  if (fields[field] === undefined || checkLogin(probe)[field] !== undefined) return fields;
  return { ...fields, [field]: undefined };
}
