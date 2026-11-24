// Secret fields start empty: the hub never returns a secret, and the form never shows a stored one (D-17, F-8).
import type { AppriseMode, SmtpSecurity, WebhookMethod } from '$lib/hub/generated/graphql';
import type { ChannelKind, NotificationChannel, NotificationTemplateInput } from '../api';
import { shownOrigin, storedTarget } from '../channels/target';

/** `clear` asks the hub to remove a stored optional secret (`""` in the input). */
export interface SecretDraft {
  value: string;
  clear: boolean;
}

export interface HeaderDraft {
  name: string;
  /** A header value is a secret (D-17). */
  value: string;
}

export interface ChannelDraft {
  name: string;
  kind: ChannelKind;
  onByDefault: boolean;
  /** False: the installation's default template (`template: null`, NOT-007). */
  ownTemplate: boolean;
  template: NotificationTemplateInput;
  mobile: { url: string; token: string };
  webhook: { url: string; method: WebhookMethod; replaceHeaders: boolean; headers: HeaderDraft[] };
  ntfy: { server: string; topic: string; token: SecretDraft; priority: number };
  email: { host: string; port: string; security: SmtpSecurity; username: string; password: SecretDraft; from: string; to: string };
  apprise: { server: string; mode: AppriseMode; urls: string; key: string; tags: string };
}

/** What the saved channel holds, as facts: never a secret's value. */
export interface Stored {
  readonly kind: ChannelKind;
  readonly headerNames: readonly string[];
  readonly ntfyToken: boolean;
  readonly emailPassword: boolean;
  readonly appriseMode: AppriseMode | null;
  /** A new target needs every stored secret again (CR-14.1). */
  readonly target: string | null;
}

export const KINDS: readonly { readonly id: ChannelKind; readonly label: string }[] = [
  { id: 'WEBHOOK', label: 'Webhook' },
  { id: 'NTFY', label: 'ntfy' },
  { id: 'EMAIL', label: 'Email' },
  { id: 'APPRISE', label: 'Apprise' },
  { id: 'MOBILE', label: 'Mobile' },
];

/** The K1 input defaults. */
const NTFY_SERVER = 'https://ntfy.sh';
const NTFY_PRIORITY = 3;
export const SMTP_PORT: Record<SmtpSecurity, number> = { STARTTLS: 587, TLS: 465 };

function emptySecret(): SecretDraft {
  return { value: '', clear: false };
}

export function newDraft(defaultTemplate: NotificationTemplateInput): ChannelDraft {
  return {
    name: '',
    kind: 'WEBHOOK',
    onByDefault: false,
    ownTemplate: false,
    template: { title: defaultTemplate.title, body: defaultTemplate.body },
    mobile: { url: '', token: '' },
    webhook: { url: '', method: 'POST', replaceHeaders: true, headers: [] },
    ntfy: { server: NTFY_SERVER, topic: '', token: emptySecret(), priority: NTFY_PRIORITY },
    email: { host: '', port: String(SMTP_PORT.STARTTLS), security: 'STARTTLS', username: '', password: emptySecret(), from: '', to: '' },
    apprise: { server: '', mode: 'STATELESS_URLS', urls: '', key: '', tags: '' },
  };
}

export function savedDraft(channel: NotificationChannel, defaultTemplate: NotificationTemplateInput): ChannelDraft {
  const draft = newDraft(channel.template ?? defaultTemplate);
  draft.name = channel.name;
  draft.kind = channel.kind;
  draft.onByDefault = channel.onByDefault;
  draft.ownTemplate = channel.template !== null;
  const { settings } = channel;
  switch (settings.__typename) {
    case 'WebhookSettings':
      draft.webhook.method = settings.method;
      draft.webhook.replaceHeaders = false;
      break;
    case 'NtfySettings':
      // Only the parsed origin is shown: a user name and password in what the hub holds never reach the field (CR-14.7).
      Object.assign(draft.ntfy, { server: shownOrigin(settings.server), priority: settings.priority });
      break;
    case 'EmailSettings':
      Object.assign(draft.email, {
        host: settings.host,
        port: String(settings.port),
        security: settings.security,
        username: settings.username ?? '',
        from: settings.from,
        to: settings.to.join(', '),
      });
      break;
    case 'AppriseSettings':
      Object.assign(draft.apprise, { server: shownOrigin(settings.server), mode: settings.mode, tags: settings.tags.join(', ') });
      break;
  }
  return draft;
}

export function storedOf(channel: NotificationChannel): Stored {
  const { settings } = channel;
  return {
    kind: channel.kind,
    headerNames: settings.__typename === 'WebhookSettings' ? settings.headerNames : [],
    ntfyToken: settings.__typename === 'NtfySettings' && settings.tokenStored,
    emailPassword: settings.__typename === 'EmailSettings' && settings.passwordStored,
    appriseMode: settings.__typename === 'AppriseSettings' ? settings.mode : null,
    target: storedTarget(settings),
  };
}

/** Empties every secret in place, so the caller can drop them from the DOM in the same task (F-8); header names are not secret. */
export function forgetSecrets(draft: ChannelDraft): void {
  draft.mobile.url = '';
  draft.mobile.token = '';
  draft.webhook.url = '';
  for (const header of draft.webhook.headers) header.value = '';
  draft.ntfy.topic = '';
  draft.ntfy.token.value = '';
  draft.email.password.value = '';
  draft.apprise.urls = '';
  draft.apprise.key = '';
}
