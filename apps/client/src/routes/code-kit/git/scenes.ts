import type { Done, Running } from '$lib/code/git/DetailsGit.svelte';
import type { SetupStage } from '$lib/code/git/WorktreeSetupCard.svelte';
import type { ChangedFile, Changes, Checkpoint, GitStatus } from '$lib/code/git/git';

/** Made states of a branch for the Git page of the kit: no runner is asked. */
const file = (path: string, additions: number, deletions: number, more: Partial<ChangedFile> = {}): ChangedFile => ({ path, kind: 'modified', additions, deletions, ...more });

const PATCH = `diff --git a/crates/daemon/src/threads/git/push.rs b/crates/daemon/src/threads/git/push.rs
index 3f1c2aa..9b7d410 100644
--- a/crates/daemon/src/threads/git/push.rs
+++ b/crates/daemon/src/threads/git/push.rs
@@ -18,9 +18,12 @@ pub fn allowed(status: &Status, default_branch_allowed: bool) -> Result<&str, GitRefusal> {
     if !status.has_remote {
         return Err(GitRefusal::NoRemote);
     }
-    if status.is_default_branch {
+    if status.behind > 0 {
+        return Err(GitRefusal::Behind);
+    }
+    if status.is_default_branch && !default_branch_allowed {
         return Err(GitRefusal::DefaultBranch);
     }
     Ok(branch)
 }
`;

export const FILES: readonly ChangedFile[] = [
  file('crates/daemon/src/threads/git/push.rs', 4, 1, { patch: PATCH }),
  file('crates/daemon/src/threads/git/status.rs', 212, 0, { kind: 'added', patch: '@@ -0,0 +1,2 @@\n+//! How the branch of a worktree stands.\n+use agent_threads::git::Status;\n' }),
  file('apps/client/src/lib/code/git/git.ts', 38, 12, { patch: "@@ -1,3 +1,3 @@\n-export type GitStep = 'commit' | 'push';\n+export type GitStep = 'commit' | 'push' | 'pull_request';\n \n export interface QuickAction {\n" }),
  file('apps/client/src/lib/code/git/old-name.ts', 0, 0, { kind: 'renamed', old_path: 'apps/client/src/lib/code/git/name.ts', path: 'apps/client/src/lib/code/git/new-name.ts', patch: 'diff --git a/apps/client/src/lib/code/git/name.ts b/apps/client/src/lib/code/git/new-name.ts\nsimilarity index 100%\nrename from apps/client/src/lib/code/git/name.ts\nrename to apps/client/src/lib/code/git/new-name.ts\n' }),
  file('docs/mockup/code-git.png', 0, 0, { binary: true }),
  file('README.md', 0, 9, { kind: 'deleted', patch: '@@ -1,2 +0,0 @@\n-# Old\n-Gone.\n' })
];

const clean: GitStatus = {
  branch: 'anima/3fa41c07',
  default_branch: 'main',
  is_default_branch: false,
  has_remote: true,
  push_url: 'https://github.com/ada/engine.git',
  upstream: 'origin/anima/3fa41c07',
  ahead: 0,
  behind: 0,
  ahead_of_default: 0,
  files: [],
  additions: 0,
  deletions: 0
};
const dirty: GitStatus = { ...clean, files: FILES, additions: 254, deletions: 22 };
const without = (status: GitStatus, key: 'branch' | 'upstream'): GitStatus => Object.fromEntries(Object.entries(status).filter(([name]) => name !== key)) as unknown as GitStatus;
const detached = without(dirty, 'branch');
const unpushed = without(clean, 'upstream');

/** One state of the Git rows of the details card. */
export interface CardScene {
  status?: GitStatus;
  running?: Running;
  done?: Done;
  failure?: string;
}

export const CARDS: Readonly<Record<string, CardScene>> = {
  loading: {},
  clean: { status: clean },
  dirty: { status: dirty },
  'dirty-default-branch': { status: { ...dirty, branch: 'main', is_default_branch: true } },
  'dirty-no-remote': { status: { ...dirty, has_remote: false } },
  unpushed: { status: { ...unpushed, ahead_of_default: 2 } },
  ahead: { status: { ...clean, ahead: 2, ahead_of_default: 2 } },
  behind: { status: { ...clean, behind: 3 } },
  diverged: { status: { ...clean, ahead: 1, behind: 3 } },
  'pushed-no-pull-request': { status: { ...clean, ahead_of_default: 2 } },
  detached: { status: detached },
  running: { status: dirty, running: { label: 'Pushing...', seconds: 72, line: 'Writing objects: 100% (14/14), 3.21 KiB' } },
  done: { status: clean, done: { title: 'Pushed 9b7d410 to anima/3fa41c07', detail: 'Git: the push guard for the default branch' } },
  failed: { status: dirty, failure: 'Only you can do this. An agent cannot commit, push, revert or open a pull request.' }
};

export const DIRTY = dirty;
export const ON_DEFAULT: GitStatus = { ...dirty, branch: 'main', is_default_branch: true };

export const CHECKPOINTS: readonly Checkpoint[] = [0, 1, 2, 3].map((turn) => ({ turn, commit: `c0ffee${String(turn)}`, at: 1_795_300_000 + turn * 600 }));

export const CHANGES: Changes = { files: FILES, additions: 254, deletions: 22, truncated: false };

/** The same files with no patch: the patches were over the limit of the runner. */
export const OVER_LIMIT: Changes = { ...CHANGES, files: FILES.map((one) => file(one.path, one.additions, one.deletions, { kind: one.kind })), truncated: true };

export const STAGES: Readonly<Record<string, readonly SetupStage[]>> = {
  running: [
    { name: 'Fetch base branch', state: 'skipped' },
    { name: 'Check out files', state: 'done', seconds: 3 },
    { name: 'Init submodules', state: 'running' },
    { name: 'Run setup script', state: 'pending' },
    { name: 'Start agent', state: 'pending' }
  ],
  ready: [
    { name: 'Fetch base branch', state: 'skipped' },
    { name: 'Check out files', state: 'done', seconds: 3 },
    { name: 'Init submodules', state: 'skipped' },
    { name: 'Run setup script', state: 'skipped' },
    { name: 'Start agent', state: 'done', seconds: 1 }
  ],
  failed: [
    { name: 'Fetch base branch', state: 'skipped' },
    { name: 'Check out files', state: 'failed' },
    { name: 'Init submodules', state: 'pending' },
    { name: 'Run setup script', state: 'pending' },
    { name: 'Start agent', state: 'pending' }
  ]
};
