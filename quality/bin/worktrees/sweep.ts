// Removes the build output (`target/`) of each linked worktree whose branch has landed:
// `bun run worktrees:sweep [--plan]`. A worktree loses its `target/` only when the tip of its branch
// is in `origin/dev`, no process works in its folder and it has no uncommitted change
// (`quality/src/worktrees/sweep.ts`). It never removes a worktree, a branch or anything but `target/`.
// With `--plan` it removes nothing and lists what it would remove.
import { lstatSync, rmSync } from "node:fs";
import { basename, join } from "node:path";
import { REPO_ROOT } from "../../src/repository.ts";
import { choose, isInside, linkedWorktrees, parseWorkingDirectories, parseWorktrees, sizeText, type Worktree } from "../../src/worktrees/sweep.ts";

const INTO = "origin/dev";

function run(argv: string[], cwd = REPO_ROOT): { code: number; out: string } {
  const done = Bun.spawnSync(argv, { cwd, stdout: "pipe", stderr: "pipe" });
  return { code: done.exitCode, out: done.stdout.toString() };
}

function stop(message: string): never {
  console.error(`worktrees:sweep: ${message}`);
  process.exit(2);
}

/** The working directories of the processes this user can see; the sweep stops when they cannot be read. */
function workingDirectories(): string[] {
  const listed = run(["lsof", "-a", "-d", "cwd", "-Fn"]);
  // lsof exits 1 when one process cannot be read; its output still lists the others.
  if (listed.code > 1 || listed.out === "") stop("the working directories of the processes cannot be read (`lsof`)");
  return parseWorkingDirectories(listed.out);
}

function hasTarget(worktree: Worktree): boolean {
  try {
    return lstatSync(join(worktree.path, "target")).isDirectory();
  } catch {
    return false;
  }
}

function kibibytes(folder: string): number {
  const measured = run(["du", "-sk", folder]);
  return measured.code === 0 ? Number.parseInt(measured.out, 10) || 0 : 0;
}

const plan = process.argv.includes("--plan");
if (run(["git", "fetch", "origin", "dev"]).code !== 0) stop("git fetch origin dev failed");
const listed = run(["git", "worktree", "list", "--porcelain"]);
if (listed.code !== 0) stop("git worktree list failed");
const directories = workingDirectories();

let total = 0;
for (const worktree of linkedWorktrees(parseWorktrees(listed.out))) {
  const name = basename(worktree.path);
  const target = !worktree.missing && hasTarget(worktree);
  const choice = choose(worktree, {
    target,
    landed: target && worktree.branch !== undefined && run(["git", "merge-base", "--is-ancestor", worktree.head, INTO]).code === 0,
    busy: directories.some((directory) => isInside(directory, worktree.path)),
    changed: target && run(["git", "status", "--porcelain", "--untracked-files=no"], worktree.path).out.trim() !== "",
  });
  if (!choice.sweep) {
    if (target) console.log(`kept     ${name}: ${choice.reason}`);
    continue;
  }
  const folder = join(worktree.path, "target");
  const size = kibibytes(folder);
  if (!plan) rmSync(folder, { recursive: true, force: true });
  total += size;
  console.log(`${plan ? "would free" : "freed"} ${sizeText(size).padStart(8)}  ${name} (${worktree.branch})`);
}
console.log(`${plan ? "would free" : "freed"} ${sizeText(total)} in all${plan ? "; run without --plan to remove" : ""}`);
