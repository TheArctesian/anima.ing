export interface LoginChallenge { url: string; code: string; session: string }
export function challenge(value: unknown, provider: 'codex' | 'claude_code'): LoginChallenge | null {
  if (!value || typeof value !== 'object') return null;
  const input = value as Record<string, unknown>;
  return provider === 'codex' ? codex(input) : claude(input);
}
function codex(input: Record<string, unknown>): LoginChallenge | null {
  if (input.state !== 'challenge' || typeof input.code !== 'string' || !/^[A-Za-z0-9-]{1,64}$/.test(input.code)
    || input.verification_url !== 'https://auth.openai.com/codex/device') return null;
  return { url: input.verification_url, code: input.code, session: '' };
}
function claude(input: Record<string, unknown>): LoginChallenge | null {
  if (input.state !== 'paste-challenge' || typeof input.session !== 'string'
    || !/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(input.session)
    || typeof input.verification_url !== 'string' || !claudeUrl(input.verification_url)) return null;
  return { url: input.verification_url, code: '', session: input.session };
}
function claudeUrl(raw: string): boolean {
  if (raw.length > 2048 || /[^\x21-\x7e]/.test(raw)) return false;
  try {
    const url = new URL(raw);
    return url.origin === 'https://claude.com' && url.pathname === '/cai/oauth/authorize'
      && !url.username && !url.password && !url.hash
      && url.searchParams.getAll('redirect_uri').length === 1
      && url.searchParams.get('redirect_uri') === 'https://platform.claude.com/oauth/code/callback';
  } catch { return false; }
}
