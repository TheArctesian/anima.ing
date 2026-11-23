/** PLN-019: the document gates on the merge commit check the comparison that a merge keeps. */
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { isGenerated, isRecord, mockupTable, MOCKUPS, recordCells, recordPath, recordText, tableRows, withMockupTable } from "../documents/mockup-records.ts";

const COMMIT = /`([0-9a-f]{7,40})`/;

function comparedAt(record: string): string | undefined {
  return COMMIT.exec(recordCells(record)?.Compared ?? "")?.[1];
}

/** When neither commit is older, `incoming` wins: the gates on the merge commit then say if its screen changed after it. */
export function newerRecord(current: string, incoming: string, older: (a: string, b: string) => boolean): string {
  const now = comparedAt(current);
  const next = comparedAt(incoming);
  if (next === undefined) return now === undefined ? incoming : current;
  if (now === undefined || now === next) return incoming;
  return older(next, now) ? current : incoming;
}

export function recordsFromTable(index: string, base: string): Map<string, string> {
  const before = new Set(tableRows(base).map((cells) => JSON.stringify(cells)));
  const records = new Map<string, string>();
  for (const cells of tableRows(index)) {
    const path = recordPath(cells.Mockup);
    if (path !== undefined && !before.has(JSON.stringify(cells))) records.set(path, recordText(cells));
  }
  return records;
}

function git(root: string, args: string[]): { ok: boolean; out: string } {
  const done = Bun.spawnSync(["git", ...args], { cwd: root, stdout: "pipe", stderr: "pipe" });
  return { ok: done.exitCode === 0, out: done.stdout.toString() };
}

export function handWritten(index: string): string {
  return withMockupTable(index, mockupTable([])) ?? index;
}

export function mergeIndex(base: string, ours: string, theirs: string): string | undefined {
  const folder = mkdtempSync(join(tmpdir(), "anima-index-"));
  try {
    const files = { ours: join(folder, "ours"), base: join(folder, "base"), theirs: join(folder, "theirs") };
    writeFileSync(files.ours, handWritten(ours));
    writeFileSync(files.base, handWritten(base));
    writeFileSync(files.theirs, handWritten(theirs));
    const done = Bun.spawnSync(["git", "merge-file", "-p", files.ours, files.base, files.theirs], { stdout: "pipe", stderr: "pipe" });
    return done.exitCode === 0 ? done.stdout.toString() : undefined;
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
}

function settledIndex(base: string | undefined, sides: (string | undefined)[]): string | undefined | "conflict" {
  const [ours, theirs] = sides;
  if (ours !== undefined && theirs !== undefined && isGenerated(ours) && isGenerated(theirs)) {
    if (ours === theirs) return ours;
    return mergeIndex(base !== undefined && isGenerated(base) ? base : ours, ours, theirs) ?? "conflict";
  }
  return sides.find((each) => each !== undefined && isGenerated(each));
}

export interface Settled {
  written: string[];
  conflict?: string;
}

/** A conflict in the parts of the index that a person writes is the one thing that it does not settle. */
export function settleMockups(root: string): Settled {
  const show = (spec: string): string | undefined => {
    const done = git(root, ["show", spec]);
    return done.ok ? done.out : undefined;
  };
  const older = (a: string, b: string): boolean => git(root, ["merge-base", "--is-ancestor", a, b]).ok;
  const base = git(root, ["merge-base", "HEAD", "MERGE_HEAD"]).out.trim();
  const written = new Map<string, string>();
  for (const side of ["HEAD", "MERGE_HEAD"]) {
    const index = show(`${side}:${MOCKUPS}`);
    if (index === undefined || isGenerated(index)) continue;
    for (const [path, incoming] of recordsFromTable(index, show(`${base}:${MOCKUPS}`) ?? "")) {
      const current = written.get(path) ?? show(`${side === "HEAD" ? "MERGE_HEAD" : "HEAD"}:${path}`);
      written.set(path, current === undefined ? incoming : newerRecord(current, incoming, older));
    }
  }
  const conflicted = git(root, ["diff", "--name-only", "--diff-filter=U"]).out.split("\n").filter(isRecord);
  for (const path of conflicted) {
    const kept = keptRecord(show(`:2:${path}`), show(`:3:${path}`), older);
    if (kept !== undefined) written.set(path, kept);
  }
  const index = settledIndex(show(`${base}:${MOCKUPS}`), [show(`HEAD:${MOCKUPS}`), show(`MERGE_HEAD:${MOCKUPS}`)]);
  if (index !== undefined && index !== "conflict") written.set(MOCKUPS, index);
  writeAll(root, written);
  if (index !== "conflict") return { written: [...written.keys()] };
  return { written: [...written.keys()], conflict: `a conflict in the part of ${MOCKUPS} that a person writes` };
}

function keptRecord(ours: string | undefined, theirs: string | undefined, older: (a: string, b: string) => boolean): string | undefined {
  if (ours === undefined || theirs === undefined) return ours ?? theirs;
  return newerRecord(ours, theirs, older);
}

function writeAll(root: string, written: Map<string, string>): void {
  for (const [path, text] of written) {
    writeFileSync(join(root, path), text);
    git(root, ["add", "--", path]);
  }
}
