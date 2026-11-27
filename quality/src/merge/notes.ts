/** The notes that the merge train keeps on `origin` (R-252): a tip that failed alone, and why a tip was left out. */

function git(root: string, args: string[]): { ok: boolean; out: string } {
  const done = Bun.spawnSync(["git", ...args], { cwd: root, stdout: "pipe", stderr: "pipe" });
  return { ok: done.exitCode === 0, out: done.stdout.toString().trim() };
}

/** A checkout fetches no notes. A note added with no local ref starts a history of its own, and `origin` refuses its push. */
function fetchNotes(root: string, ref: string): void {
  git(root, ["fetch", "--quiet", "origin", `+${ref}:${ref}`]);
}

/** The text of each note of `ref` on `origin`, by the commit that it is on. */
export function readNotes(root: string, ref: string): Map<string, string> {
  fetchNotes(root, ref);
  const noted = git(root, ["notes", "--ref", ref, "list"]).out.split("\n").map((line) => line.split(" ")[1] ?? "");
  return new Map(noted.filter((sha) => sha !== "").map((sha) => [sha, git(root, ["notes", "--ref", ref, "show", sha]).out]));
}

/** Puts the note on the notes that `origin` holds; false when `origin` does not have it. */
export function saveNote(root: string, ref: string, sha: string, text: string): boolean {
  fetchNotes(root, ref);
  return git(root, ["notes", "--ref", ref, "add", "--force", "-m", text, sha]).ok && git(root, ["push", "--quiet", "origin", ref]).ok;
}
