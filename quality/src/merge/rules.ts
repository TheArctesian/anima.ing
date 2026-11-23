/** R-198, PLN-019, PLN-020, PLN-021. */
import { isRecord, MOCKUPS } from "../documents/mockup-records.ts";

export const GENERATED = ["docs/plan/register.md", "docs/operator-queue.md"] as const;

/** The operator's switches in `quality/merge-rule.json`: `fullGates` (R-NEW-gaterule), `mergeWithoutCheck` (R-NEW-automerge). */
export interface MergeRule {
  fullGates: "mac" | "github";
  mergeWithoutCheck: boolean;
  /** A passed train still lands when `dev` moved under it only in documents. */
  carryDocuments: boolean;
}

export function parseMergeRule(text: string): MergeRule | string {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return "the rule file is not JSON";
  }
  const rule = value as Partial<MergeRule> | null;
  if (rule === null || typeof rule !== "object") return "the rule file is not an object";
  if (rule.fullGates !== "mac" && rule.fullGates !== "github") return "fullGates must be \"mac\" or \"github\"";
  if (typeof rule.mergeWithoutCheck !== "boolean") return "mergeWithoutCheck must be true or false";
  if (rule.mergeWithoutCheck && rule.fullGates !== "github") return "mergeWithoutCheck needs fullGates \"github\"";
  return { fullGates: rule.fullGates, mergeWithoutCheck: rule.mergeWithoutCheck, carryDocuments: rule.carryDocuments === true };
}

export function conflictsForAHand(conflicted: string[]): string[] {
  return conflicted.filter((path) => !(GENERATED as readonly string[]).includes(path) && path !== MOCKUPS && !isRecord(path));
}

export function mergeMessage(branch: string, into: string, numbers: [string, string][]): string {
  const subject = `Merge ${branch} into ${into}`;
  if (numbers.length === 0) return `${subject}\n`;
  const lines = numbers.map(([placeholder, number]) => `${placeholder} is ${number}`);
  return `${subject}\n\nNumbers assigned at this merge (PLN-021):\n${lines.join("\n")}\n`;
}

export interface CheckRun {
  name: string;
  status: string;
  conclusion: string | null;
}

/** The job of the workflow `gates` that passes when each other job passed. */
const LAST_JOB = "passed";

export function gatesRefusal(runs: CheckRun[]): string | undefined {
  const last = runs.filter((run) => run.name === LAST_JOB || run.name.endsWith(`/ ${LAST_JOB}`));
  if (last.length === 0) return "the commit has no run of the workflow `gates` (job `passed`)";
  const bad = last.find((run) => run.status !== "completed" || run.conclusion !== "success");
  return bad === undefined ? undefined : `the job \`passed\` of the gates is ${bad.status}, ${bad.conclusion ?? "no conclusion"}`;
}

/** Merges by union (`.gitattributes`). */
export const DECISIONS = "docs/plan/decisions.md";

/** A union merge keeps both versions of a row that two branches changed. */
export function rowsTwice(text: string): string[] {
  const seen = new Set<string>();
  const twice = new Set<string>();
  for (const line of text.split("\n")) {
    const id = /^\| ([A-Z][A-Za-z0-9-]*-[A-Za-z0-9-]+) \|/.exec(line)?.[1];
    if (id === undefined) continue;
    if (seen.has(id)) twice.add(id);
    seen.add(id);
  }
  return [...twice];
}


/** These merge by union too. */
export const FINDINGS = "docs/security-findings.md";
export const OWNERS = "docs/plan/ownership.md";

/** A table ends at the first line that is not a row, so an identifier can be in two tables of one document. */
export function twiceInOneTable(text: string, key: (cells: string[]) => string | undefined): string[] {
  let seen = new Set<string>();
  const twice = new Set<string>();
  for (const line of text.split("\n")) {
    if (!line.startsWith("|")) {
      seen = new Set();
      continue;
    }
    const id = key(line.split("|").slice(1, -1).map((cell) => cell.trim()));
    if (id === undefined) continue;
    if (seen.has(id)) twice.add(id);
    seen.add(id);
  }
  return [...twice];
}

export function headingsTwice(text: string): string[] {
  const all = [...text.matchAll(/^### ([A-Z]+-[A-Za-z0-9-]+):/gm)].map((match) => match[1] ?? "");
  return [...new Set(all.filter((id, at) => all.indexOf(id) !== at))];
}

export function findingKey(cells: string[]): string | undefined {
  return /^SF-[A-Za-z0-9-]+$/.exec(cells[0] ?? "")?.[0];
}

export function ownerKey(cells: string[]): string | undefined {
  const identifiers = cells.at(-1) ?? "";
  return /\d/.test(identifiers) ? identifiers : undefined;
}
