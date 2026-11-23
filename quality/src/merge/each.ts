/**
 * `bin/merge.ts --each`: merge named branches into `dev` one at a time, while other sessions push
 * claims to `dev` every few minutes. `startOf` and `outcomeOf` are the pure rules; `mergeEach` runs
 * git and `bin/merge.ts`.
 */

/** How many times one branch is merged again after `dev` moved during its merge. */
export const ATTEMPTS = 3;

/** What to do with a branch before its merge. */
export type Start = "absent" | "in-dev" | "merge";

/** A branch that origin does not have is absent; a tip that `dev` holds needs no merge. */
export function startOf(onOrigin: boolean, inDev: boolean): Start {
  if (!onOrigin) return "absent";
  return inDev ? "in-dev" : "merge";
}

/** One run of `bin/merge.ts <branch> --push`: its exit code and the commit of `origin/dev` before and after it. */
export interface Attempt {
  exit: number;
  devBefore: string;
  devAfter: string;
  /** 1 for the first run. */
  number: number;
}

export type Outcome = "merged" | "again" | "stopped";

/**
 * A run that passed merged and pushed the branch. A run that failed while `dev` moved is run again,
 * up to `ATTEMPTS`: the push was refused, or the merge used a `dev` that is old now. A run that
 * failed on a `dev` that did not move failed for the branch (gates, a conflict), and a new run
 * gives the same answer.
 */
export function outcomeOf(attempt: Attempt): Outcome {
  if (attempt.exit === 0) return "merged";
  const moved = attempt.devAfter !== attempt.devBefore;
  return moved && attempt.number < ATTEMPTS ? "again" : "stopped";
}

function git(root: string, ...args: string[]): { ok: boolean; out: string } {
  const done = Bun.spawnSync(["git", ...args], { cwd: root, stdout: "pipe", stderr: "pipe" });
  return { ok: done.exitCode === 0, out: done.stdout.toString().trim() };
}

/** The commit of `dev` on origin now. */
function originDev(root: string): string {
  if (!git(root, "fetch", "--quiet", "origin").ok) throw new Error("git fetch failed");
  return git(root, "rev-parse", "origin/dev").out;
}

/** Merges one branch with `bin/merge.ts <branch> --push`, again while `dev` moves under it. */
function mergeOne(root: string, branch: string): { ok: boolean; line: string } {
  originDev(root);
  const start = startOf(git(root, "rev-parse", "--quiet", "--verify", `origin/${branch}`).ok, git(root, "merge-base", "--is-ancestor", `origin/${branch}`, "origin/dev").ok);
  if (start === "absent") return { ok: false, line: `${branch}: not a branch on origin` };
  if (start === "in-dev") return { ok: true, line: `${branch}: dev holds it already` };
  for (let number = 1; ; number += 1) {
    const devBefore = originDev(root);
    const exit = Bun.spawnSync(["bun", "quality/bin/merge.ts", branch, "--push"], { cwd: root, stdout: "inherit", stderr: "inherit" }).exitCode;
    const outcome = outcomeOf({ exit, devBefore, devAfter: originDev(root), number });
    if (outcome === "merged") return { ok: true, line: `${branch}: merged and pushed` };
    if (outcome === "stopped") return { ok: false, line: `${branch}: not merged (bin/merge.ts exit ${String(exit)}, attempt ${String(number)})` };
    console.log(`merge --each: dev moved during the merge of ${branch}; merging it again on the new dev`);
  }
}

/**
 * `bun quality/bin/merge.ts --each <branch>...`: merges each branch that origin has and `dev` does not
 * hold, one at a time, each with its own push; prints one line for each branch. True when each merged
 * or was in `dev` already. It never forces a push and never pushes to `main`.
 */
export function mergeEach(root: string, branches: string[]): boolean {
  const lines = branches.map((branch) => mergeOne(root, branch.replace(/^origin\//, "")));
  for (const { line } of lines) console.log(`merge --each: ${line}`);
  return lines.every(({ ok }) => ok);
}
