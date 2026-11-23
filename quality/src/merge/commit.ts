// The checks run on the merge commit: the mockup check (PLN-019) compares with HEAD, which is the old `dev` until the commit.

function git(root: string, args: string[]): { ok: boolean; out: string } {
  const done = Bun.spawnSync(["git", ...args], { cwd: root, stdout: "pipe", stderr: "pipe" });
  return { ok: done.exitCode === 0, out: done.stdout.toString().trim() };
}

export function undoMerge(root: string, before: string): void {
  git(root, ["merge", "--abort"]);
  git(root, ["reset", "--hard", "--quiet", before]);
  git(root, ["clean", "-fd", "--quiet"]);
}

/** A failure of `check` removes the commit and is returned. */
export function commitThenCheck(root: string, message: string, check: () => string | undefined): string | undefined {
  const before = git(root, ["rev-parse", "HEAD"]).out;
  const failure = git(root, ["add", "-A"]).ok && git(root, ["commit", "--quiet", "-m", message]).ok ? check() : "the merge commit failed";
  if (failure !== undefined) undoMerge(root, before);
  return failure;
}
