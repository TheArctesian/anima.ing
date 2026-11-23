// Merges named branches into `dev` one at a time: `bun quality/bin/merge-each.ts <branch>...`, in a
// clean worktree that is on `dev` (as `bin/merge.ts`). For each branch that origin has and `dev` does
// not hold, it runs `bin/merge.ts <branch> --push`, which refuses a branch whose gates did not pass on
// GitHub and pushes only to `dev`. Other sessions push claims to `dev` every few minutes: when `dev`
// moved during a merge, the merge runs again on the new `dev` (`src/merge/each.ts`). It never forces
// a push and never pushes to `main`. The merge train (`bin/train.ts`) stays the normal way; this is
// for a session that merges branches by hand.
import { outcomeOf, startOf } from "../src/merge/each.ts";
import { REPO_ROOT } from "../src/repository.ts";

function git(...args: string[]): { ok: boolean; out: string } {
  const done = Bun.spawnSync(["git", ...args], { cwd: REPO_ROOT, stdout: "pipe", stderr: "pipe" });
  return { ok: done.exitCode === 0, out: done.stdout.toString().trim() };
}

/** The commit of `dev` on origin now. */
function originDev(): string {
  if (!git("fetch", "--quiet", "origin").ok) throw new Error("git fetch failed");
  return git("rev-parse", "origin/dev").out;
}

/** Merges one branch, again while `dev` moves under it; returns the line that says what happened. */
function mergeOne(branch: string): { ok: boolean; line: string } {
  originDev();
  const start = startOf(git("rev-parse", "--quiet", "--verify", `origin/${branch}`).ok, git("merge-base", "--is-ancestor", `origin/${branch}`, "origin/dev").ok);
  if (start === "absent") return { ok: false, line: `${branch}: not a branch on origin` };
  if (start === "in-dev") return { ok: true, line: `${branch}: dev holds it already` };
  for (let number = 1; ; number += 1) {
    const devBefore = originDev();
    const exit = Bun.spawnSync(["bun", "quality/bin/merge.ts", branch, "--push"], { cwd: REPO_ROOT, stdout: "inherit", stderr: "inherit" }).exitCode;
    const outcome = outcomeOf({ exit, devBefore, devAfter: originDev(), number });
    if (outcome === "merged") return { ok: true, line: `${branch}: merged and pushed` };
    if (outcome === "stopped") return { ok: false, line: `${branch}: not merged (bin/merge.ts exit ${String(exit)}, attempt ${String(number)})` };
    console.log(`merge-each: dev moved during the merge of ${branch}; merging it again on the new dev`);
  }
}

const branches = process.argv.slice(2).map((each) => each.replace(/^origin\//, ""));
if (branches.length === 0) {
  console.error("merge-each: usage: bun quality/bin/merge-each.ts <branch>...");
  process.exit(2);
}
const lines = branches.map(mergeOne);
for (const { line } of lines) console.log(`merge-each: ${line}`);
process.exit(lines.every(({ ok }) => ok) ? 0 : 1);
