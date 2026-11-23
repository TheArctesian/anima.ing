import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { commitThenCheck } from "./commit.ts";
import { settleMockups } from "./mockups.ts";
import { conflictsForAHand, GENERATED, mergeMessage } from "./rules.ts";

const CHECKS = [["bun", "run", "quality:documents"], ["bun", "quality/bin/numbers.ts", "--check"]];

export type Finished = { numbers: [string, string][] } | { failure: string };

/** With a failure nothing is committed; the caller puts the worktree back. */
export function finishMerge(root: string, source: string, into: string): Finished {
  let said = "";
  const run = (argv: string[]): { ok: boolean; out: string } => {
    const done = Bun.spawnSync(argv, { cwd: root, stdout: "pipe", stderr: "pipe" });
    said = done.stderr.toString();
    process.stderr.write(said);
    return { ok: done.exitCode === 0, out: done.stdout.toString() };
  };
  const settled = settleMockups(root);
  if (settled.conflict !== undefined) return { failure: settled.conflict };
  for (const path of GENERATED) {
    const ours = run(["git", "show", `HEAD:${path}`]);
    if (!ours.ok) return { failure: `git show HEAD:${path} failed` };
    writeFileSync(join(root, path), ours.out);
  }
  if (!run(["bun", "install", "--frozen-lockfile"]).ok) return { failure: "`bun install --frozen-lockfile` failed" };
  const assigned = run(["bun", "quality/bin/numbers.ts", "--assign"]);
  if (!assigned.ok) return { failure: "`bun quality/bin/numbers.ts --assign` failed" };
  const numbers = assigned.out
    .split("\n")
    .map((line) => line.split(" -> "))
    .filter((pair): pair is [string, string] => pair.length === 2);
  if (!run(["bun", "run", "quality:register"]).ok) return { failure: "`bun run quality:register` failed" };
  const failed = commitThenCheck(root, mergeMessage(source, into, numbers), () => CHECKS.find((argv) => !run(argv).ok)?.join(" "));
  return failed === undefined ? { numbers } : { failure: `\`${failed}\` failed on the merge commit${firstFault(said)}` };
}

export function firstFault(said: string): string {
  // `bun run` first prints the command that it runs, after "$ ".
  const line = said.split("\n").map((each) => each.trim()).find((each) => each !== "" && !each.startsWith("$ "));
  return line === undefined ? "" : `: ${line.length > 300 ? `${line.slice(0, 300)}...` : line}`;
}

/** The generated files take the side of HEAD; `bun quality/bin/register.ts` then writes them again. */
export function settleGenerated(root: string): string | undefined {
  const git = (args: string[]) => Bun.spawnSync(["git", ...args], { cwd: root, stdout: "pipe", stderr: "pipe" });
  const conflicted = git(["diff", "--name-only", "--diff-filter=U"]).stdout.toString().split("\n").filter((path) => path !== "");
  const forAHand = conflictsForAHand(conflicted);
  if (forAHand.length > 0) return `a conflict in ${forAHand.join(", ")}`;
  const settled = settleMockups(root);
  if (settled.conflict !== undefined) return settled.conflict;
  for (const path of GENERATED.filter((each) => conflicted.includes(each))) {
    if (git(["checkout", "--ours", "--", path]).exitCode !== 0 || git(["add", "--", path]).exitCode !== 0) return `${path} could not take the side of HEAD`;
  }
  return undefined;
}
