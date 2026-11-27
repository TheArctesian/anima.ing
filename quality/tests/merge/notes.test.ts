import { describe, expect, test } from "bun:test";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { readNotes, saveNote } from "../../src/merge/notes.ts";

/** These tests make repositories with many git commands; a busy machine needs more than the 5 seconds of a test. */
const SLOW = 60_000;
const REF = "refs/notes/train";

function git(root: string, ...args: string[]): string {
  const done = Bun.spawnSync(["git", ...args], { cwd: root });
  if (done.exitCode !== 0) throw new Error(`git ${args.join(" ")}: ${done.stderr.toString()}`);
  return done.stdout.toString().trim();
}

/** `git clone` fetches the branches and the tags, as the checkout of the workflow `merge-to-dev` does: no notes. */
function clone(origin: string, name: string): string {
  const root = join(dirname(origin), name);
  git(dirname(origin), "clone", "--quiet", origin, root);
  git(root, "config", "user.name", "test");
  git(root, "config", "user.email", "test@anima.invalid");
  return root;
}

describe("R-252 the notes of the train stay on origin", () => {
  test("a note saved in a new checkout keeps each note that origin holds", () => {
    const folder = mkdtempSync(join(tmpdir(), "anima-notes-"));
    const origin = join(folder, "origin.git");
    git(folder, "init", "--quiet", "--bare", "--initial-branch=dev", origin);
    const first = clone(origin, "first");
    git(first, "switch", "--quiet", "-c", "dev");
    const [one, two] = ["one.txt", "two.txt"].map((path) => {
      writeFileSync(join(first, path), `${path}\n`);
      git(first, "add", "-A");
      git(first, "commit", "--quiet", "-m", path);
      return git(first, "rev-parse", "HEAD");
    });
    git(first, "push", "--quiet", "origin", "dev");
    expect(saveNote(first, REF, one ?? "", "the first refusal")).toBe(true);

    const fresh = clone(origin, "fresh");
    expect(saveNote(fresh, REF, two ?? "", "the second refusal")).toBe(true);

    expect(readNotes(clone(origin, "reader"), REF)).toEqual(
      new Map([
        [one ?? "", "the first refusal"],
        [two ?? "", "the second refusal"],
      ]),
    );
  }, SLOW);
});
