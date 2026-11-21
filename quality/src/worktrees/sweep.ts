// Which linked worktrees may lose their build output (`target/`) because their branch has landed.
// The program `bin/worktrees/sweep.ts` reads git and the processes; this module only decides.

/** One entry of `git worktree list --porcelain`. */
export interface Worktree {
  path: string;
  head: string;
  /** The branch without `refs/heads/`; undefined for a detached HEAD. */
  branch: string | undefined;
  /** The folder of the worktree is gone (`prunable`). */
  missing: boolean;
}

/** What is known of one worktree when the choice is made. */
export interface Facts {
  /** The tip of the branch is in `origin/dev`. */
  landed: boolean;
  /** A process has the folder of the worktree, or a folder in it, as its working directory. */
  busy: boolean;
  /** Tracked files of the worktree have changes that are not committed. */
  changed: boolean;
  /** `target/` is a real folder (not a link) in the worktree. */
  target: boolean;
}

export type Choice = { sweep: true } | { sweep: false; reason: string };

/** Parses `git worktree list --porcelain`. The first entry is the main worktree. */
export function parseWorktrees(porcelain: string): Worktree[] {
  const worktrees: Worktree[] = [];
  for (const block of porcelain.split(/\n\n+/)) {
    const lines = block.split("\n").filter((line) => line !== "");
    const path = lines.find((line) => line.startsWith("worktree "))?.slice("worktree ".length);
    if (path === undefined) continue;
    const head = lines.find((line) => line.startsWith("HEAD "))?.slice("HEAD ".length) ?? "";
    const ref = lines.find((line) => line.startsWith("branch "))?.slice("branch ".length);
    worktrees.push({
      path,
      head,
      branch: ref?.replace(/^refs\/heads\//, ""),
      missing: lines.some((line) => line === "prunable" || line.startsWith("prunable ")),
    });
  }
  return worktrees;
}

/** The linked worktrees: every entry but the first, which is the main worktree. */
export function linkedWorktrees(worktrees: Worktree[]): Worktree[] {
  return worktrees.slice(1);
}

/**
 * Whether the `target/` of a worktree may be removed. Only a branch whose tip is in `origin/dev`,
 * with no process in its folder and no uncommitted change, loses its build output: a branch that
 * was just made from `dev` has its tip in `dev` too, and a change shows that work is under way.
 */
export function choose(worktree: Worktree, facts: Facts): Choice {
  if (worktree.missing) return { sweep: false, reason: "its folder is gone" };
  if (worktree.branch === undefined) return { sweep: false, reason: "it has no branch (detached HEAD)" };
  if (!facts.target) return { sweep: false, reason: "it has no target/ folder" };
  if (!facts.landed) return { sweep: false, reason: `${worktree.branch} is not in origin/dev` };
  if (facts.busy) return { sweep: false, reason: "a process works in its folder" };
  if (facts.changed) return { sweep: false, reason: "it has uncommitted changes" };
  return { sweep: true };
}

/** Whether a working directory is the folder of the worktree or a folder in it. */
export function isInside(directory: string, folder: string): boolean {
  const base = folder.replace(/\/+$/, "");
  return directory === base || directory.startsWith(`${base}/`);
}

/** The working directories in the output of `lsof -a -d cwd -Fn` (lines `n<path>`). */
export function parseWorkingDirectories(lsof: string): string[] {
  return lsof
    .split("\n")
    .filter((line) => line.startsWith("n"))
    .map((line) => line.slice(1));
}

/** A size in kibibytes as text, in GB or MB. */
export function sizeText(kibibytes: number): string {
  const gigabytes = kibibytes / (1024 * 1024);
  return gigabytes >= 1 ? `${gigabytes.toFixed(1)} GB` : `${Math.round(kibibytes / 1024)} MB`;
}
