// This module only decides; `bin/worktrees/sweep.ts` reads git and the processes.

/** One entry of `git worktree list --porcelain`. */
export interface Worktree {
  path: string;
  head: string;
  branch: string | undefined;
  missing: boolean;
}

export interface Facts {
  landed: boolean;
  busy: boolean;
  changed: boolean;
  /** `target/` is a real folder, not a link. */
  target: boolean;
}

export type Choice = { sweep: true } | { sweep: false; reason: string };

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

export function linkedWorktrees(worktrees: Worktree[]): Worktree[] {
  return worktrees.slice(1);
}

/** A branch just made from `dev` has its tip in `dev` too; an uncommitted change shows that work is under way. */
export function choose(worktree: Worktree, facts: Facts): Choice {
  if (worktree.missing) return { sweep: false, reason: "its folder is gone" };
  if (worktree.branch === undefined) return { sweep: false, reason: "it has no branch (detached HEAD)" };
  if (!facts.target) return { sweep: false, reason: "it has no target/ folder" };
  if (!facts.landed) return { sweep: false, reason: `${worktree.branch} is not in origin/dev` };
  if (facts.busy) return { sweep: false, reason: "a process works in its folder" };
  if (facts.changed) return { sweep: false, reason: "it has uncommitted changes" };
  return { sweep: true };
}

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

export function sizeText(kibibytes: number): string {
  const gigabytes = kibibytes / (1024 * 1024);
  return gigabytes >= 1 ? `${gigabytes.toFixed(1)} GB` : `${Math.round(kibibytes / 1024)} MB`;
}
