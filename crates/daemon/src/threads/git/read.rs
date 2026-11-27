//! Count-only Git reads from the authenticated local runner.

use agent_threads::git::read::{Changes, Checkpoint, Outcome, Read, Refusal, Status};
use agent_threads::git::{ChangeKind, GitRefusal, Scope};

use super::changes::{self, Look};
use super::place::Worktree;
use super::run::Git;
use super::{Engine, Target, checkpoint, project, status};

impl Engine {
    pub(in crate::threads) fn registered(&self, target: Target<'_>) -> bool {
        self.home.worktree(target.thread, target.repository).is_ok()
    }

    pub(in crate::threads) async fn read_summary(&self, target: Target<'_>, read: Read) -> Outcome {
        self.summary(target, read)
            .await
            .unwrap_or_else(|why| Outcome::Refused {
                why: match why {
                    GitRefusal::NoWorktree | GitRefusal::BadPath => Refusal::NoWorktree,
                    GitRefusal::UnknownCheckpoint => Refusal::UnknownCheckpoint,
                    _ => Refusal::Unavailable,
                },
            })
    }

    async fn summary(&self, target: Target<'_>, read: Read) -> Result<Outcome, GitRefusal> {
        self.closed()?;
        let _index = self.index(target).await;
        self.closed()?;
        let worktree = self.home.worktree(target.thread, target.repository)?;
        let git = self.git.for_worktree(&worktree).await?;
        match read {
            Read::Status => branch(&git, &worktree).await,
            Read::Checkpoints { limit, after } => turns(&git, &worktree, limit, after).await,
            Read::Changes { scope } => changes(&git, &worktree, scope).await,
        }
    }
}

async fn branch(git: &Git, worktree: &Worktree) -> Result<Outcome, GitRefusal> {
    let listed = project::settings(git, &worktree.layout(), worktree.folder()).await?;
    let status = status::of(git, worktree, &listed).await?;
    Ok(Outcome::Status {
        status: Status {
            detached: status.branch.is_none(),
            is_default_branch: status.is_default_branch,
            has_remote: status.has_remote,
            has_upstream: status.upstream.is_some(),
            ahead: status.ahead,
            behind: status.behind,
            ahead_of_default: status.ahead_of_default,
            files: status.files.len(),
            additions: status.additions,
            deletions: status.deletions,
        },
    })
}

async fn turns(
    git: &Git,
    worktree: &Worktree,
    limit: u32,
    after: Option<u32>,
) -> Result<Outcome, GitRefusal> {
    let limit = limit.clamp(1, 100) as usize;
    let mut remaining = checkpoint::list(git, worktree)
        .await?
        .into_iter()
        .filter(|turn| after.is_none_or(|after| turn.turn > after));
    let checkpoints: Vec<_> = remaining
        .by_ref()
        .take(limit)
        .map(|turn| Checkpoint {
            turn: turn.turn,
            at: turn.at,
        })
        .collect();
    let next_cursor = remaining
        .next()
        .and_then(|_| checkpoints.last().map(|turn| turn.turn));
    Ok(Outcome::Checkpoints {
        checkpoints,
        next_cursor,
    })
}

async fn changes(git: &Git, worktree: &Worktree, scope: Scope) -> Result<Outcome, GitRefusal> {
    let read = changes::of(git, worktree, scope, Look::Counts).await?;
    let mut counts = Changes {
        scope,
        files: read.files.len(),
        added: 0,
        modified: 0,
        deleted: 0,
        renamed: 0,
        binary: 0,
        additions: read.additions,
        deletions: read.deletions,
    };
    for file in read.files {
        match file.kind {
            ChangeKind::Added => counts.added += 1,
            ChangeKind::Modified => counts.modified += 1,
            ChangeKind::Deleted => counts.deleted += 1,
            ChangeKind::Renamed => counts.renamed += 1,
        }
        counts.binary += usize::from(file.binary);
    }
    Ok(Outcome::Changes { changes: counts })
}
