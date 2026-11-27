/** R-252. */

export const TRAIN = "train/";
/** A failed train of n branches costs about log2(n) more runs. */
export const MOST = 12;

/** A run of the workflow `gates`, as `gh run list` gives it. */
export interface GatesRun {
  databaseId: number;
  headBranch: string;
  headSha: string;
  event: string;
  status: string;
  conclusion: string | null;
  updatedAt: string;
}

export interface Tip {
  branch: string;
  sha: string;
}

/** The tip of a branch that failed alone in a train, and when (ISO time). */
export type Refused = ReadonlyMap<string, string>;

function isWork(branch: string): boolean {
  return branch !== "dev" && branch !== "main" && branch !== "HEAD" && !branch.startsWith(TRAIN);
}

export interface Standing {
  tip: Tip;
  passedAt?: string;
  reason?: string;
}

function pushRuns(tip: Tip, runs: GatesRun[]): GatesRun[] {
  return runs.filter((run) => run.event === "push" && run.headBranch === tip.branch);
}

function gatesOf(tip: Tip, runs: GatesRun[]): { passedAt?: string; reason?: string } {
  const all = pushRuns(tip, runs);
  const mine = all.filter((run) => run.headSha === tip.sha);
  const passedAt = mine.filter((run) => run.status === "completed" && run.conclusion === "success").map((run) => run.updatedAt).sort().at(-1);
  if (passedAt !== undefined) return { passedAt };
  if (mine.some((run) => run.status !== "completed")) return { reason: "its gates still run" };
  const last = mine.sort((one, other) => (one.updatedAt < other.updatedAt ? 1 : -1))[0];
  if (last !== undefined) return { reason: `its gates ended with "${last.conclusion ?? "no conclusion"}"; correct the branch or run them again` };
  const older = all.find((run) => run.conclusion === "success");
  if (older !== undefined) return { reason: `its gates passed for an older commit (${older.headSha.slice(0, 8)}), not for its tip` };
  return { reason: "its gates did not run for its tip" };
}

export function standing(tips: Tip[], runs: GatesRun[], refused: Refused, inDev: (sha: string) => boolean): Standing[] {
  return tips
    .filter((tip) => isWork(tip.branch) && !inDev(tip.sha))
    .map((tip) => {
      const gates = gatesOf(tip, runs);
      const failedAt = refused.get(tip.sha);
      if (gates.passedAt !== undefined && failedAt !== undefined && failedAt >= gates.passedAt) {
        return { tip, reason: `it failed alone in a train at ${failedAt}; run its gates again or push a correction` };
      }
      return { tip, ...gates };
    });
}

/** Each tip that waits for a train: its own gates passed and no train refused it; the oldest pass first, then the name. */
export function queue(tips: Tip[], runs: GatesRun[], refused: Refused, inDev: (sha: string) => boolean): Tip[] {
  const ready = standing(tips, runs, refused, inDev).flatMap((each) => (each.reason === undefined && each.passedAt !== undefined ? [{ tip: each.tip, at: each.passedAt }] : []));
  ready.sort((a, b) => (a.at === b.at ? (a.tip.branch < b.tip.branch ? -1 : 1) : a.at < b.at ? -1 : 1));
  return ready.map((each) => each.tip);
}

/** A tip that does not merge with `dev`, and the files of the conflict. */
export interface Conflict {
  tip: Tip;
  paths: string[];
}

export interface Batch {
  taken: Tip[];
  conflicts: Conflict[];
}

/** The first `MOST` tips of the queue that merge with `dev`. A tip with a conflict takes no place; `conflict` is asked again at the next batch. */
export function batch(waiting: Tip[], conflict: (tip: Tip) => string[]): Batch {
  const taken: Tip[] = [];
  const conflicts: Conflict[] = [];
  for (const tip of waiting) {
    if (taken.length === MOST) break;
    const paths = conflict(tip);
    if (paths.length === 0) taken.push(tip);
    else conflicts.push({ tip, paths });
  }
  return { taken, conflicts };
}

export type Verdict = "start-gates" | "wait" | "land" | "split" | "build-again";

export function verdict(trainSha: string, runs: GatesRun[]): Verdict {
  const mine = runs.filter((run) => run.headSha === trainSha && run.headBranch.startsWith(TRAIN));
  if (mine.length === 0) return "start-gates";
  if (mine.some((run) => run.status !== "completed")) return "wait";
  if (mine.some((run) => run.conclusion === "success")) return "land";
  return mine.some((run) => run.conclusion === "failure" || run.conclusion === "timed_out") ? "split" : "build-again";
}

export function firstHalf<T>(members: T[]): T[] {
  return members.slice(0, Math.ceil(members.length / 2));
}

export function trainName(devSha: string, now: Date): string {
  return `${TRAIN}${devSha.slice(0, 8)}-${now.toISOString().replace(/[-:]|\.\d+Z$/g, "").toLowerCase()}`;
}

export function mergedBranch(subject: string): string | undefined {
  return /^Merge origin\/(\S+) into dev$/.exec(subject)?.[1];
}

export function oneLine(reason: string): string {
  return reason.replace(/\s*\n\s*/g, " ").trim();
}

export function leftOutSaid(branch: string, note: string | undefined): string | undefined {
  if (note === undefined || note.trim() === "") return undefined;
  const [at = "", ...reason] = note.trim().split("\n");
  return `${branch} was left out of the train at ${at}: ${oneLine(reason.join(" "))}`;
}
