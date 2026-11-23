/** A train carried onto a moved `dev` lands only when each file that is not a document is the one of `tested` (R-NEW-traincarry). */
import { undoMerge } from "./commit.ts";
import { stack } from "./stack.ts";
import type { Tip } from "./train.ts";

function git(root: string, args: string[]): { ok: boolean; out: string } {
  const done = Bun.spawnSync(["git", ...args], { cwd: root, stdout: "pipe", stderr: "pipe" });
  return { ok: done.exitCode === 0, out: done.stdout.toString().trim() };
}

export function changed(root: string, from: string, to: string): string[] {
  return git(root, ["diff", "--name-only", from, to]).out.split("\n").filter((path) => path !== "");
}

export interface Rules {
  finish: (tip: Tip) => string | undefined;
  isDocument: (path: string) => boolean;
}

export interface Passed {
  into: string;
  tested: string;
  held: Tip[];
}

export interface Carried {
  outcome: "landed" | "again" | "different";
  refused: string[];
}

/** "again": the result was right but `into` moved during the push. Each outcome but "landed" restores the worktree. */
export function carry(root: string, { into, tested, held }: Passed, rules: Rules): Carried {
  const start = git(root, ["rev-parse", "HEAD"]).out;
  // A branch that a person merged into `dev` while the gates ran is there already.
  const waiting = held.filter((tip) => !git(root, ["merge-base", "--is-ancestor", tip.sha, "HEAD"]).ok);
  const made = stack(root, waiting, rules.finish);
  const refused = [
    ...made.leftOut.map((each) => `${each.branch}: ${each.reason}`),
    ...changed(root, tested, "HEAD").filter((path) => !rules.isDocument(path)),
  ];
  if (refused.length === 0 && git(root, ["push", "--quiet", "origin", `HEAD:refs/heads/${into}`]).ok) return { outcome: "landed", refused };
  undoMerge(root, start);
  return { outcome: refused.length === 0 ? "again" : "different", refused };
}
