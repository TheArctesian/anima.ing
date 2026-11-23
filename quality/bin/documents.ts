import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { checkDocuments, MOCKUP, parseKnown } from "../src/documents/documents.ts";
import { placeholdersInScreens } from "../src/documents/mockups.ts";
import { QUEUE } from "../src/documents/queue.ts";
import { REGISTER } from "../src/documents/register.ts";
import { layoutChangedSince } from "../src/documents/screens.ts";
import { isShared, placeholders } from "../src/merge/numbers.ts";
import { DECISIONS, findingKey, FINDINGS, headingsTwice, ownerKey, OWNERS, rowsTwice, twiceInOneTable } from "../src/merge/rules.ts";
import { listMeasurableFiles, readRepoFile, REPO_ROOT } from "../src/repository.ts";
import { report } from "../src/report.ts";

// A path can be in the list more than once while a merge has conflicts.
const paths = [...new Set(listMeasurableFiles(REPO_ROOT))];
const documents = paths
  .filter((path) => path.endsWith(".md") || MOCKUP.test(path))
  .map((path) => ({ path, text: path.endsWith(".md") ? readRepoFile(REPO_ROOT, path) : "" }));
const known = parseKnown(readRepoFile(REPO_ROOT, "quality/duplicate-identifiers.txt"));

const exists = (path: string): boolean => existsSync(join(REPO_ROOT, path));
const violations = checkDocuments(documents, exists, known, (screen, commit) => layoutChangedSince(REPO_ROOT, screen, commit));
// A file that does not exist is not in the list of `checkDocuments`.
for (const [path, signal] of [[REGISTER, "register-without-list"], [QUEUE, "queue-without-list"]] as const) {
  if (!exists(path)) violations.push({ path, signal, detail: "the file is missing" });
}
violations.push(...placeholdersInScreens(documents, paths, (path) => readRepoFile(REPO_ROOT, path)));
// PLN-021: a branch can hold a placeholder of a number; `dev` and `main` cannot.
const head = spawnSync("git", ["rev-parse", "--abbrev-ref", "HEAD"], { cwd: REPO_ROOT, encoding: "utf8" }).stdout.trim();
if (isShared(head) || isShared(process.env["GITHUB_REF_NAME"] ?? "")) {
  for (const placeholder of placeholders(documents)) {
    violations.push({ path: "docs", signal: "number-not-assigned", detail: `${placeholder}: run the merge script` });
  }
}
for (const id of rowsTwice(documents.find((document) => document.path === DECISIONS)?.text ?? "")) {
  violations.push({ path: DECISIONS, signal: "decision-row-twice", detail: `${id}: keep one row` });
}
const textOf = (path: string): string => documents.find((document) => document.path === path)?.text ?? "";
for (const id of [...twiceInOneTable(textOf(FINDINGS), findingKey), ...headingsTwice(textOf(FINDINGS))]) {
  violations.push({ path: FINDINGS, signal: "finding-twice", detail: `${id}: keep one row and one entry` });
}
for (const id of twiceInOneTable(textOf(OWNERS), ownerKey)) {
  violations.push({ path: OWNERS, signal: "owner-row-twice", detail: `${id}: keep one row` });
}
process.exit(report("documents", violations));
