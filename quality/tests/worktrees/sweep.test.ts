import { describe, expect, test } from "bun:test";
import { choose, isInside, linkedWorktrees, parseWorkingDirectories, parseWorktrees, sizeText, type Facts, type Worktree } from "../../src/worktrees/sweep.ts";

const PORCELAIN = [
  "worktree /work/anima.ing\nHEAD 1111111111111111111111111111111111111111\nbranch refs/heads/main\n",
  "worktree /work/wt-landed\nHEAD 2222222222222222222222222222222222222222\nbranch refs/heads/fix/landed\n",
  "worktree /work/wt-proof\nHEAD 3333333333333333333333333333333333333333\ndetached\n",
  "worktree /work/wt-gone\nHEAD 4444444444444444444444444444444444444444\nbranch refs/heads/fix/gone\nprunable gitdir file points to non-existent location\n",
].join("\n");

const landed: Worktree = { path: "/work/wt-landed", head: "2".repeat(40), branch: "fix/landed", missing: false };
const free: Facts = { landed: true, busy: false, changed: false, target: true };

describe("the sweep of the build output of landed worktrees", () => {
  test("it reads each worktree of git, and leaves the main worktree out", () => {
    const all = parseWorktrees(PORCELAIN);
    expect(all.map((each) => [each.path, each.branch, each.missing])).toEqual([
      ["/work/anima.ing", "main", false],
      ["/work/wt-landed", "fix/landed", false],
      ["/work/wt-proof", undefined, false],
      ["/work/wt-gone", "fix/gone", true],
    ]);
    expect(linkedWorktrees(all).map((each) => each.path)).toEqual(["/work/wt-landed", "/work/wt-proof", "/work/wt-gone"]);
  });

  test("only a landed branch with no process in its folder and no change loses its target/", () => {
    expect(choose(landed, free)).toEqual({ sweep: true });
    expect(choose(landed, { ...free, landed: false })).toEqual({ sweep: false, reason: "fix/landed is not in origin/dev" });
    expect(choose(landed, { ...free, busy: true })).toEqual({ sweep: false, reason: "a process works in its folder" });
    expect(choose(landed, { ...free, changed: true })).toEqual({ sweep: false, reason: "it has uncommitted changes" });
    expect(choose(landed, { ...free, target: false })).toEqual({ sweep: false, reason: "it has no target/ folder" });
  });

  test("a worktree with no branch, or whose folder is gone, is never swept", () => {
    expect(choose({ ...landed, branch: undefined }, free).sweep).toBe(false);
    expect(choose({ ...landed, missing: true }, free).sweep).toBe(false);
  });

  test("a process in the folder or in a folder of it makes the worktree busy; a neighbour with a longer name does not", () => {
    const directories = parseWorkingDirectories("p101\nfcwd\nn/work/wt-landed/crates/hub\np102\nfcwd\nn/work/wt-landed-2\n");
    expect(directories).toEqual(["/work/wt-landed/crates/hub", "/work/wt-landed-2"]);
    expect(isInside(directories[0] ?? "", "/work/wt-landed")).toBe(true);
    expect(isInside(directories[1] ?? "", "/work/wt-landed")).toBe(false);
    expect(isInside("/work/wt-landed", "/work/wt-landed/")).toBe(true);
  });

  test("a size is given in GB from one GB, else in MB", () => {
    expect(sizeText(14 * 1024 * 1024)).toBe("14.0 GB");
    expect(sizeText(138_000)).toBe("135 MB");
  });
});
