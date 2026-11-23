import type { ConnectAttempt } from './api';

export function attempt(more: Partial<ConnectAttempt> = {}): ConnectAttempt {
  return {
    id: 'attempt-1',
    phase: 'WAITING_FOR_BROWSER',
    startedBy: 'USER',
    expiresAt: '2026-11-13T12:55:00Z',
    pageUrl: 'https://hub.example/connect/github',
    appSlug: null,
    ownerLogin: null,
    installUrl: null,
    appSettingsUrl: null,
    failure: null,
    message: null,
    account: null,
    ...more,
  };
}

interface Held {
  connect: ConnectAttempt | null;
  readonly link: string;
}

export function connectAnswer(held: Held, operation: string): Record<string, unknown> | undefined {
  if (operation === 'StartGithubConnect') {
    held.connect = attempt();
    return { startGithubConnect: held.connect };
  }
  if (operation === 'OpenGithubConnect') return { openGithubConnect: { url: held.link, expiresAt: '2026-11-13T12:05:00Z' } };
  if (operation === 'CancelGithubConnect') {
    held.connect = { ...(held.connect ?? attempt()), phase: 'CANCELLED' };
    return { cancelGithubConnect: held.connect };
  }
  return undefined;
}
