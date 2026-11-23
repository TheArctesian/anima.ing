import type { GithubAppInput } from '../api';

/** `privateKey` is a secret: it lives in the dialog until the form is sent, then is emptied. */
export interface ConnectForm {
  label: string;
  appId: string;
  installationId: string;
  privateKey: string;
}

export const EMPTY_FORM: Readonly<ConnectForm> = { label: '', appId: '', installationId: '', privateKey: '' };

export type ConnectField = 'label' | 'appId' | 'installationId' | 'privateKey';
export interface ConnectProblem {
  readonly field: ConnectField;
  readonly message: string;
}

/** Fixed words: a key never appears in a message (PRJ-018, LOG-007). */
export const KEY_FILE_UNREADABLE = 'Could not read that file. Paste the key instead.';
export const KEY_FILE_TOO_LARGE = 'Too large for a private key. Choose the .pem file from GitHub.';
const MAX_KEY_BYTES = 16 * 1024;

const DIGITS = /^\d+$/;
const BLOCK = /^(-----BEGIN [A-Z ]+-----)\s*([\s\S]*?)\s*(-----END [A-Z ]+-----)$/;

/** A password field removes line breaks from a paste; the hub expects PEM lines of 64 characters. */
export function restorePem(text: string): string {
  const trimmed = text.trim();
  const block = BLOCK.exec(trimmed);
  if (block === null || trimmed.includes('\n')) return trimmed;
  const [, begin = '', body = '', end = ''] = block;
  const lines = body.replaceAll(/\s+/g, '').match(/.{1,64}/g) ?? [];
  return [begin, ...lines, end].join('\n');
}

export function keyFileProblem(file: Pick<File, 'size'>): string | null {
  return file.size > MAX_KEY_BYTES ? KEY_FILE_TOO_LARGE : null;
}

/** SRC-003: both IDs are digits only. */
export function checkConnect(form: Readonly<ConnectForm>): { ok: true; input: GithubAppInput } | { ok: false; problems: readonly ConnectProblem[] } {
  const problems: ConnectProblem[] = [];
  const label = form.label.trim();
  const appId = form.appId.trim();
  const installationId = form.installationId.trim();
  if (label === '' || label.length > 64) problems.push({ field: 'label', message: 'Name: 1 to 64 characters.' });
  if (!DIGITS.test(appId)) problems.push({ field: 'appId', message: 'The App ID is digits only.' });
  if (!DIGITS.test(installationId)) problems.push({ field: 'installationId', message: 'The installation ID is digits only.' });
  if (form.privateKey.trim() === '') problems.push({ field: 'privateKey', message: 'Paste the private key or choose its .pem file.' });
  if (problems.length > 0) return { ok: false, problems };
  return { ok: true, input: { label, appId, installationId, privateKey: restorePem(form.privateKey) } };
}
