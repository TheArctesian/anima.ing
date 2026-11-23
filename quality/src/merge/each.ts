/**
 * The rules of `bin/merge-each.ts`: merge named branches into `dev` one at a time, while other
 * sessions push claims to `dev` every few minutes. Pure.
 */

/** How many times one branch is merged again after `dev` moved during its merge. */
export const ATTEMPTS = 3;

/** What to do with a branch before its merge. */
export type Start = "absent" | "in-dev" | "merge";

/** A branch that origin does not have is absent; a tip that `dev` holds needs no merge. */
export function startOf(onOrigin: boolean, inDev: boolean): Start {
  if (!onOrigin) return "absent";
  return inDev ? "in-dev" : "merge";
}

/** One run of `bin/merge.ts <branch> --push`: its exit code and the commit of `origin/dev` before and after it. */
export interface Attempt {
  exit: number;
  devBefore: string;
  devAfter: string;
  /** 1 for the first run. */
  number: number;
}

export type Outcome = "merged" | "again" | "stopped";

/**
 * A run that passed merged and pushed the branch. A run that failed while `dev` moved is run again,
 * up to `ATTEMPTS`: the push was refused, or the merge used a `dev` that is old now. A run that
 * failed on a `dev` that did not move failed for the branch (gates, a conflict), and a new run
 * gives the same answer.
 */
export function outcomeOf(attempt: Attempt): Outcome {
  if (attempt.exit === 0) return "merged";
  const moved = attempt.devAfter !== attempt.devBefore;
  return moved && attempt.number < ATTEMPTS ? "again" : "stopped";
}
