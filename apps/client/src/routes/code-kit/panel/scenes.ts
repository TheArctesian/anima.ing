import type { Details } from '$lib/code/details/model';
import type { FolderListing } from '$lib/code/panel/files/crumbs';
import { NO_TABS, opened, type PanelTabs } from '$lib/code/panel/tabs';

export const FULL: Details = {
  runContext: {
    environments: [
      { id: 'studio', label: 'studio', kind: 'desktop' },
      { id: 'nas', label: 'nas', kind: 'server' },
    ],
    environment: 'studio',
    mode: 'local',
    worktreePath: '/Users/ada/worktrees/fix-login',
    path: '/Users/ada/worktrees/fix-login',
    locked: false,
  },
  openIn: {
    editors: [
      { id: 'zed', label: 'Zed' },
      { id: 'vscode', label: 'VS Code' },
      { id: 'file-manager', label: 'Finder' },
    ],
    preferred: 'zed',
  },
  scripts: {
    scripts: [
      { id: 'dev', name: 'dev', icon: 'play', shortcut: '⌘⇧D' },
      { id: 'test', name: 'test', icon: 'test', onSettle: true },
      { id: 'install', name: 'install', icon: 'configure', onSetup: true },
    ],
    primary: 'dev',
  },
  branch: { name: 'fix/login-redirect' },
  pullRequests: [
    { number: 412, title: 'Login: keep the page the person came from after the sign-in', state: 'open', checks: { done: 7, total: 9, failed: 0 }, watching: true },
    { number: 398, title: 'Session cookie: one name for each hub', state: 'merged' },
    { number: 377, title: 'Sign-in page: first draft', state: 'closed' },
  ],
  issues: [{ key: 'ANI-231', title: 'After the sign-in the app opens the first page, not the page of the link', href: 'https://example.com/ANI-231', comments: 4 }],
  servers: [
    { id: 'vite', port: 5173, startedBy: 'agent', process: 'vite', canStop: true, canForward: true },
    { id: 'hub', port: 8080, startedBy: 'terminal', process: 'anima-hub', canStop: true, canForward: true, forwardedTo: 8080 },
  ],
  automations: [
    { id: 'nightly', title: 'Nightly test run', schedule: 'Every day at 02:00', enabled: true, lastRun: 'succeeded', next: 'in 5 hours' },
    { id: 'deps', title: 'Dependency check', schedule: 'Mondays', enabled: false, lastRun: 'failed' },
  ],
  related: [
    { id: 'parent', kind: 'parent', title: 'Plan the new sign-in flow', status: 'done' },
    { id: 'fork', kind: 'fork', title: 'Try the redirect with a cookie', status: 'idle' },
    { id: 'a1', kind: 'subagent', title: 'Run the browser tests', status: 'running', seconds: 95, canStop: true },
    { id: 'a2', kind: 'subagent', title: 'Read the session code', status: 'done', seconds: 41 },
    { id: 'a3', kind: 'subagent', title: 'Check the migration', status: 'failed' },
    { id: 's1', kind: 'side_chat', title: 'What does the cookie hold?', status: 'idle' },
  ],
  usage: {
    limits: [
      { label: '5 hours', left: 64, timeLeft: 40, pace: 'ahead', resetsIn: 7200, resetsAt: '16:00' },
      { label: 'Weekly', left: 17, timeLeft: 35, pace: 'under', resetsIn: 200_000, resetsAt: 'Tue 09:00' },
      { label: 'Weekly, Opus', left: 6, timeLeft: 35, pace: 'under', resetsIn: 200_000, resetsAt: 'Tue 09:00' },
    ],
  },
  context: { used: 128_400, max: 200_000, compactsAt: 170_000 },
};

export const PLAIN: Details = {
  runContext: { environments: [], mode: 'worktree', locked: true },
  scripts: { scripts: [] },
  branch: { name: 'main', base: true },
  pullRequests: [{ number: 12 }],
  usage: { unavailable: 'This account has no subscription limits.' },
  context: { used: 9500 },
};

export const REFS = [
  { name: 'fix/login-redirect', current: true },
  { name: 'main', isDefault: true },
  { name: 'feature/usage-page', worktree: true },
  { name: 'origin/release-2026-11', remote: true },
  { name: 'origin/old-experiment', remote: true, disabled: true },
];

export const SOURCE = `<script lang="ts">
  import { goto } from '$app/navigation';

  /** After the sign-in the person goes back to the page that the link named. */
  export function back(from: string | null): string {
    return from !== null && from.startsWith('/') && !from.startsWith('//') ? from : '/';
  }
</script>

<form method="post" onsubmit={() => goto(back(new URLSearchParams(location.search).get('from')))}>
  <label>Hub <input name="hub" autocomplete="url" /></label>
  <button>Sign in</button>
</form>
`;

export const MARKDOWN = `---
title: Sign-in
---

# Sign-in

The sign-in page keeps the page that the person came from.

- The address is in the query \`from\`.
- Only a path of this application is accepted.

\`\`\`ts
back('/devices'); // '/devices'
back('//evil.example'); // '/'
\`\`\`
`;

export const TABS: PanelTabs = opened(opened(NO_TABS, { id: 'files', kind: 'files', title: 'Files' }), { id: 'file:login', kind: 'file', title: '+page.svelte', path: 'src/routes/sign-in/+page.svelte' });

export function listing(folder: string): FolderListing {
  if (folder.endsWith('sign-in')) {
    return { state: 'ready', entries: [{ name: '+page.svelte', path: 'src/routes/sign-in/+page.svelte', folder: false }, { name: '+page.ts', path: 'src/routes/sign-in/+page.ts', folder: false }] };
  }
  return { state: 'ready', entries: [{ name: 'sign-in', path: 'src/routes/sign-in', folder: true }, { name: 'devices', path: 'src/routes/devices', folder: true }, { name: '+layout.svelte', path: 'src/routes/+layout.svelte', folder: false }] };
}

export const PICTURE = `data:image/svg+xml,${encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="640" height="400"><rect width="640" height="400" fill="steelblue"/><circle cx="320" cy="200" r="120" fill="white"/></svg>')}`;

export const PATCH = `diff --git a/src/routes/sign-in/back.ts b/src/routes/sign-in/back.ts
index 3f1c2aa..9b7d410 100644
--- a/src/routes/sign-in/back.ts
+++ b/src/routes/sign-in/back.ts
@@ -1,10 +1,12 @@
 import { goto } from '$app/navigation';
 
-/** After the sign-in the person goes to the first page. */
-export function back(): string {
-  return '/';
+/** After the sign-in the person goes back to the page that the link named. */
+export function back(from: string | null): string {
+  const own = from !== null && from.startsWith('/') && !from.startsWith('//');
+  return own ? from : '/';
 }
 
 export async function leave(search: string): Promise<void> {
-  await goto(back());
+  const from = new URLSearchParams(search).get('from');
+  await goto(back(from));
 }
`;
