import { spawnSync } from "node:child_process";

/** Compares with HEAD, not the worktree: a merge that is not committed is not in it. */
export function layoutChangedSince(root: string, screen: string, commit: string): number | undefined {
  const changed = spawnSync("git", ["diff", "--name-only", `${commit}..HEAD`, "--", `:(literal)${screen}`], { cwd: root, encoding: "utf8" });
  if (changed.status !== 0) return undefined;
  return changed.stdout.split("\n").filter((path) => path.endsWith(".svelte") || path.endsWith(".css")).length;
}
